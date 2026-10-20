use std::fs;
use std::io;
use std::net::SocketAddrV4;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{
    ActiveProfiles, AlertService, Coordinator, DataFolder, DnsService, DnsServiceParts,
    EngineParts, FileWatcher, GuardSource, Inventory, Job, ProfileSet, ProfileStates, Scheduler,
    SchedulerWaker, SettingsFiles, TrafficSampler, WireGuardTunnel,
};
use crate::alerts::AlertTexts;
use crate::clock::Clock;
use crate::config::{Settings, StateFile};
use crate::dns::{Forwarder, NrptSync, QueryHandler, QueryHandlerParts, ResolverSet, ServerSet};
use crate::list::{ListSet, SharedProfileMatcher};
use crate::logs::{Log, LogFile};
use crate::platform::composition::PlatformSet;
use crate::routes::HostClaims;
use crate::stats::QueryStats;
use crate::tunnel::{EndpointAddresses, EndpointResolver, Tunnel};

/// How an engine runs: its data folder, the proxy address (`--listen`), the profiles to connect
/// besides the restored ones (`--connect`), and whether `connected=` is restored regardless of
/// `connect_at_boot` (`install --restore`).
#[derive(Clone, Debug)]
pub struct EngineConfig {
    pub folder: DataFolder,
    pub listen: Option<SocketAddrV4>,
    pub connect: Vec<String>,
    pub restore_all: bool,
}

/// The composition root of the logic: builds everything from a `PlatformSet`, does the start
/// cleanup and restore, and starts the scheduler. `stop` disconnects everything but
/// keeps `connected=`.
pub struct Engine {
    coordinator: Arc<Coordinator>,
    sampler: Arc<TrafficSampler>,
    scheduler: Scheduler,
    folder: DataFolder,
    log: Log,
}

impl Engine {
    pub fn start(
        config: EngineConfig,
        platform: PlatformSet,
        clock: Arc<dyn Clock>,
        log: Log,
    ) -> io::Result<Self> {
        let folder = config.folder.clone();
        let coordinator = Self::prepare(config, platform, clock.clone(), &log)?;
        let parts = coordinator.parts();
        let sampler = Arc::new(TrafficSampler::new(
            parts.active.clone(),
            parts.tunnel.clone(),
            clock,
        ));
        let scheduler = Scheduler::start(Self::jobs(&coordinator, &sampler), log.clone())?;
        Ok(Self {
            coordinator,
            sampler,
            scheduler,
            folder,
            log,
        })
    }

    /// Everything but the scheduler: the data folder and its marker, the start cleanup, the
    /// files read, and the restore. Tests drive the periodic work by hand from here.
    pub fn prepare(
        config: EngineConfig,
        platform: PlatformSet,
        clock: Arc<dyn Clock>,
        log: &Log,
    ) -> io::Result<Arc<Coordinator>> {
        let folder = config.folder.clone();
        let settings_files = SettingsFiles::new(folder.clone(), platform.dry_run);
        let settings = settings_files.global()?;
        let listen = config
            .listen
            .or_else(|| settings.as_ref().ok().map(|s| s.listen))
            .unwrap_or_else(|| Settings::default().listen);
        log.info(format!(
            "SplitWire {}{} starting: data in {}, proxy on {listen}",
            env!("CARGO_PKG_VERSION"),
            if platform.dry_run { " (dry run)" } else { "" },
            folder.root().display()
        ));
        fs::create_dir_all(folder.logs())?;
        platform.folder.restrict(folder.root())?;
        log.write_to(LogFile::in_folder(&folder.logs()));
        let crashed = folder.running_marker().exists();
        folder.write(&folder.running_marker(), b"")?;
        if crashed {
            log.warn("the last run did not stop cleanly: every profile in connected= comes back");
        }
        let parts = Arc::new(Self::parts(
            &platform,
            folder.clone(),
            settings_files,
            listen,
            &clock,
            log,
        ));
        Self::clean_start(&parts);
        let inventory = Self::inventory(&platform, folder, settings, &clock)?;
        let coordinator = Arc::new(Coordinator::new(parts, inventory));
        coordinator.reload_settings();
        coordinator.reload_lists();
        coordinator.maintain();
        coordinator.restore(crashed || config.restore_all, &config.connect);
        Ok(coordinator)
    }

    pub fn coordinator(&self) -> &Arc<Coordinator> {
        &self.coordinator
    }

    pub fn sampler(&self) -> &Arc<TrafficSampler> {
        &self.sampler
    }

    /// Runs every periodic job at once (resume from sleep).
    pub fn waker(&self) -> SchedulerWaker {
        self.scheduler.waker()
    }

    /// Disconnects every profile (keeping `connected=`), which leaves no adapter, route or NRPT
    /// rule behind, then removes the crash marker.
    pub fn stop(self) {
        self.log.info("stopping");
        self.scheduler.stop();
        self.coordinator.disconnect_all(true);
        if let Err(e) = fs::remove_file(self.folder.running_marker()) {
            self.log
                .error(format!("removing the crash marker failed: {e}"));
        }
        self.log.info("stopped");
    }

    fn parts(
        platform: &PlatformSet,
        folder: DataFolder,
        settings_files: SettingsFiles,
        listen: SocketAddrV4,
        clock: &Arc<dyn Clock>,
        log: &Log,
    ) -> EngineParts {
        let fallback_servers = Arc::new(ServerSet::default());
        let fallback = Forwarder::new(platform.exchange.clone(), fallback_servers.clone());
        let seen = Arc::new(EndpointAddresses::default());
        let tunnel: Arc<dyn Tunnel> = Arc::new(WireGuardTunnel::new(
            platform.driver.clone(),
            platform.addressing.clone(),
            EndpointResolver::new(fallback.clone(), seen.clone()),
            clock.clone(),
            log.clone(),
        ));
        let active = Arc::new(ActiveProfiles::default());
        let resolvers = Arc::new(ResolverSet::default());
        let matcher = Arc::new(SharedProfileMatcher::default());
        let claims = Arc::new(Mutex::new(HostClaims::default()));
        let policy = Arc::default();
        let alerts = Arc::new(AlertService::new(clock.clone(), log.clone()));
        let stats = Arc::new(QueryStats::default());
        let handler = QueryHandler::new(
            QueryHandlerParts {
                matcher: matcher.clone(),
                resolvers: resolvers.clone(),
                policy: Arc::clone(&policy),
                fallback: fallback.clone(),
                claims: claims.clone(),
                stats: stats.clone(),
            },
            log.clone(),
        );
        let dns = Arc::new(DnsService::new(
            handler,
            DnsServiceParts {
                active: active.clone(),
                claims: claims.clone(),
                alerts: alerts.clone(),
                flusher: platform.flusher.clone(),
                clock: clock.clone(),
            },
            log.clone(),
        ));
        EngineParts {
            folder,
            settings_files,
            tunnel,
            route_table: platform.routes.clone(),
            flusher: platform.flusher.clone(),
            bound: platform.bound.clone(),
            nrpt: NrptSync::new(platform.nrpt.clone(), log.with_source("nrpt")),
            nrpt_policy: platform.nrpt_policy.clone(),
            guards: GuardSource::new(
                platform.probe.clone(),
                fallback_servers,
                seen.clone(),
                log.clone(),
            ),
            endpoints: EndpointResolver::new(fallback, seen),
            active,
            resolvers,
            matcher,
            claims,
            policy,
            alerts,
            stats,
            states: Arc::new(ProfileStates::default()),
            dns,
            listen,
            clock: clock.clone(),
            log: log.clone(),
        }
    }

    /// Every start first removes every SplitWire NRPT rule (a crash may have left some), then
    /// flushes the Windows DNS cache. Adapters and routes cannot outlive the process.
    fn clean_start(parts: &EngineParts) {
        if let Err(e) = parts.nrpt.remove_all() {
            parts
                .alerts
                .raise(AlertTexts::nrpt_write_failed(&e.to_string()));
        }
        parts.flush();
    }

    fn inventory(
        platform: &PlatformSet,
        folder: DataFolder,
        settings: Result<Settings, Vec<crate::config::SettingProblem>>,
        clock: &Arc<dyn Clock>,
    ) -> io::Result<Inventory> {
        let state = StateFile::parse(&folder.read_text(&folder.state())?);
        let profiles = ProfileSet::load(folder, platform.secrets.clone(), clock.clone())?;
        Ok(Inventory {
            profiles,
            settings,
            lists: ListSet::default(),
            state,
            proxy: None,
        })
    }

    fn jobs(coordinator: &Arc<Coordinator>, sampler: &Arc<TrafficSampler>) -> Vec<Job> {
        let watcher = FileWatcher::new(coordinator.clone());
        let (watch, maintain) = (coordinator.clone(), coordinator.clone());
        let sampler = sampler.clone();
        vec![
            Job {
                name: "files",
                period: Duration::from_secs(1),
                run: Box::new(move || watcher.tick()),
            },
            Job {
                name: "traffic",
                period: Duration::from_secs(1),
                run: Box::new(move || sampler.tick()),
            },
            Job {
                name: "tunnel watch",
                period: Duration::from_secs(10),
                run: Box::new(move || watch.watch_tunnels()),
            },
            Job {
                name: "maintenance",
                period: Duration::from_secs(30),
                run: Box::new(move || maintain.maintain()),
            },
        ]
    }
}

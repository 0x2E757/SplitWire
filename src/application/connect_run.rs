use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use super::{ActiveProfile, DnsProxy, EngineParts, Inventory, Teardown};
use crate::alerts::AlertTexts;
use crate::config::ProfileSettings;
use crate::dns::ProfileResolver;
use crate::dns::wire::{DnsQuery, TYPE_SOA};
use crate::net::Ipv4Net;
use crate::profile::{Profile, ProfileFacts, ProfileName, ProfileState};
use crate::routes::RouteManager;
use crate::stats::TrafficSeries;
use crate::tunnel::{
    AttemptKind, ConnectAttempt, ConnectStep, HandshakeHealth, HandshakePolicy, HandshakeWait,
    StepResult, TunnelLink, TunnelSpec, UpStage,
};

enum Outcome {
    Done(String),
    Skipped(String),
}

/// One Connect: the ten steps in order, each with its undo. On a failure the steps
/// done are undone in reverse, and the other profiles are not touched. Step 7 waits for the
/// peer's first handshake, so NRPT (step 9) sends no name to a tunnel that cannot carry it yet;
/// a peer without a keepalive is probed first, since only traffic makes it handshake. A Connect
/// without a handshake still succeeds, with `handshake_stale` raised.
pub struct ConnectRun<'a> {
    parts: &'a EngineParts,
    inventory: &'a mut Inventory,
    name: ProfileName,
    attempt: ConnectAttempt,
    profile: Option<Profile>,
    settings: ProfileSettings,
    upstream: Vec<SocketAddrV4>,
    spec: Option<TunnelSpec>,
    link: Option<TunnelLink>,
    /// `Tunnel::up` failed at its peer: step 4 is done, step 5 fails with this.
    peer_error: Option<io::Error>,
    up_at: SystemTime,
    /// Step 7 waited in vain.
    missed_handshake: bool,
    started_proxy: bool,
}

impl<'a> ConnectRun<'a> {
    const PROBE_ID: u16 = 0x5157;

    pub fn new(parts: &'a EngineParts, inventory: &'a mut Inventory, name: ProfileName) -> Self {
        let attempt = ConnectAttempt::new(parts.clock.now(), AttemptKind::Connect);
        Self {
            parts,
            inventory,
            name,
            attempt,
            profile: None,
            settings: ProfileSettings::default(),
            upstream: Vec::new(),
            spec: None,
            link: None,
            peer_error: None,
            up_at: parts.clock.now(),
            missed_handshake: false,
            started_proxy: false,
        }
    }

    /// The attempt, as `Err` when a step failed (everything done is undone then).
    pub fn run(mut self) -> Result<ConnectAttempt, ConnectAttempt> {
        let name = self.name.to_string();
        let log = self.parts.log.with_profile(&name);
        let mut done = Vec::new();
        for step in ConnectStep::ALL {
            self.parts
                .states
                .set(&name, ProfileState::Connecting { step });
            match self.step(step) {
                Ok(Outcome::Done(detail)) => {
                    self.attempt.set(step, StepResult::Done, detail);
                    done.push(step);
                }
                Ok(Outcome::Skipped(detail)) => {
                    self.attempt.set(step, StepResult::Skipped, detail);
                }
                Err(error) => {
                    log.error(format!(
                        "connect failed at step {} ({step}): {error}",
                        step.number()
                    ));
                    self.attempt.set(step, StepResult::Failed, error);
                    // The failed step may have left part of its work behind.
                    self.undo(step);
                    for step in done.into_iter().rev() {
                        let detail = self.undo(step);
                        self.attempt.set(step, StepResult::Undone, detail);
                    }
                    return Err(self.attempt);
                }
            }
        }
        if self.missed_handshake {
            let waited = HandshakeWait::WAIT.as_secs();
            let raised = AlertTexts::handshake_missing(&name, waited);
            self.parts.alerts.raise(raised);
        }
        log.info("connected");
        Ok(self.attempt)
    }

    fn step(&mut self, step: ConnectStep) -> Result<Outcome, String> {
        match step {
            ConnectStep::Profile => self.profile(),
            ConnectStep::Proxy => self.proxy(),
            ConnectStep::Guard => self.guard(),
            ConnectStep::Adapter => self.adapter(),
            ConnectStep::Peer => self.peer(),
            ConnectStep::Routes => self.routes(),
            ConnectStep::Dns => self.dns(),
            ConnectStep::Matcher => self.matcher(),
            ConnectStep::Nrpt => self.nrpt(),
            ConnectStep::State => self.state(),
        }
    }

    /// Undoes `step`; returns what was undone, for the step table.
    fn undo(&mut self, step: ConnectStep) -> &'static str {
        let name = self.name.to_string();
        let mut teardown = Teardown::new(self.parts, self.inventory);
        match step {
            ConnectStep::Profile => {
                self.profile = None;
                self.spec = None;
                "keys zeroed again"
            }
            ConnectStep::Proxy => {
                if self.started_proxy {
                    teardown.stop_proxy_if_idle();
                }
                "the proxy stopped"
            }
            ConnectStep::Guard => {
                teardown.unguard(&name);
                "never-routes as before"
            }
            ConnectStep::Adapter => {
                teardown.untunnel(&name);
                "adapter removed"
            }
            ConnectStep::Peer => "removed with the adapter",
            ConnectStep::Routes => {
                teardown.unroute(&name);
                "routes removed"
            }
            ConnectStep::Dns => {
                teardown.unresolve(&name);
                "resolver removed"
            }
            ConnectStep::Matcher => {
                teardown.unmatch(&name);
                "names fall through again"
            }
            ConnectStep::Nrpt => {
                teardown.unnrpt(&name);
                "rules as before"
            }
            ConnectStep::State => {
                teardown.unstate(&name);
                "removed from connected="
            }
        }
    }

    /// 1: the settings files valid, the keys decrypted, an upstream DNS known.
    fn profile(&mut self) -> Result<Outcome, String> {
        if let Err(problems) = &self.inventory.settings {
            return Err(format!("splitwire.conf is invalid: {}", problems[0]));
        }
        let settings = self
            .parts
            .settings_files
            .profile(&self.name)
            .map_err(|e| e.to_string())?
            .map_err(|problems| problems[0].to_string())?;
        let profile = self
            .inventory
            .profiles
            .open(&self.name)
            .map_err(|e| e.to_string())?;
        self.upstream = if settings.upstream.is_empty() {
            let dns = &profile.facts.dns;
            dns.iter().map(|ip| SocketAddrV4::new(*ip, 53)).collect()
        } else {
            settings.upstream.clone()
        };
        if self.upstream.is_empty() {
            return Err(
                "no upstream DNS: the profile has no DNS= and profile.conf sets no upstream".into(),
            );
        }
        self.settings = settings;
        self.profile = Some(profile);
        Ok(Outcome::Done(format!(
            "keys decrypted; upstream {}",
            Self::join(&self.upstream)
        )))
    }

    /// 2: the first connected profile starts the proxy.
    fn proxy(&mut self) -> Result<Outcome, String> {
        if let Some(proxy) = &self.inventory.proxy {
            return Ok(Outcome::Skipped(format!(
                "the proxy already runs on {}",
                proxy.address()
            )));
        }
        let proxy = DnsProxy::start(
            self.parts.listen,
            self.parts.dns.clone(),
            self.parts.log.clone(),
        )
        .map_err(|e| e.to_string())?;
        let detail = format!("bound {} UDP and TCP", proxy.address());
        self.inventory.proxy = Some(proxy);
        self.started_proxy = true;
        Ok(Outcome::Done(detail))
    }

    /// 3: the base guard refreshed, the endpoint resolved through the fallback, this tunnel a
    /// never-route of the others.
    fn guard(&mut self) -> Result<Outcome, String> {
        self.parts.guards.refresh();
        let profile = self.profile.as_ref().expect("step 1 done");
        let facts = &profile.facts;
        let host = facts.endpoint.host.to_string();
        let addresses = self.parts.endpoints.resolve(&host).map_err(|e| {
            let fallback: Vec<String> = self
                .parts
                .guards
                .fallback_servers()
                .iter()
                .map(ToString::to_string)
                .collect();
            format!(
                "{host} did not resolve through the fallback DNS {}: {e}",
                fallback.join(", ")
            )
        })?;
        let endpoint = SocketAddrV4::new(addresses[0], facts.endpoint.port);
        let subnet = facts.address.subnet();
        self.spec = Some(Self::spec(
            &self.name,
            profile,
            &self.settings,
            host.clone(),
            endpoint,
        ));
        self.parts.guards.add_tunnel(self.name.as_str(), subnet);
        self.parts.apply_guards();
        Ok(Outcome::Done(format!(
            "endpoint {host} is {endpoint}; {subnet} is a never-route of the other profiles"
        )))
    }

    /// 4: the adapter created with its address, MTU and DNS settings (step 5 configures its
    /// peer; one `Tunnel::up` does both, and its error tells which failed).
    fn adapter(&mut self) -> Result<Outcome, String> {
        let spec = self.spec.as_ref().expect("step 3 done");
        self.up_at = self.parts.clock.now();
        match self.parts.tunnel.up(spec) {
            Ok(link) => {
                self.link = Some(link);
                Ok(Outcome::Done(format!(
                    "{}: LUID {:#x}, index {}, {}/32, MTU {}",
                    spec.adapter_name(),
                    link.luid,
                    link.if_index,
                    spec.address,
                    spec.mtu
                )))
            }
            Err(error) if UpStage::of(&error) == Some(UpStage::Peer) => {
                let detail = format!("{} created", spec.adapter_name());
                self.peer_error = Some(error);
                Ok(Outcome::Done(detail))
            }
            Err(error) => Err(error.to_string()),
        }
    }

    /// 5: the peer configured and the adapter up.
    fn peer(&mut self) -> Result<Outcome, String> {
        if let Some(error) = self.peer_error.take() {
            return Err(error.to_string());
        }
        let spec = self.spec.as_ref().expect("step 3 done");
        Ok(Outcome::Done(format!(
            "peer {}, endpoint {}, keepalive {}; up",
            ProfileFacts::short_key(&spec.peer_public_key),
            spec.endpoint,
            spec.keepalive
        )))
    }

    /// 6: the /32s to the upstream DNS servers and the static entries, on this adapter.
    fn routes(&mut self) -> Result<Outcome, String> {
        let link = self.link.expect("step 4 done");
        self.parts
            .route_table
            .list_owned(link.luid)
            .map_err(|e| format!("the routes of LUID {:#x} cannot be read: {e}", link.luid))?;
        let spec = self.spec.take().expect("step 3 done");
        let settings = self.inventory.settings.as_ref().expect("checked at step 1");
        let now = self.parts.clock.now();
        let name = self.name.to_string();
        let upstream_ips: Vec<Ipv4Addr> = self.upstream.iter().map(|s| *s.ip()).collect();
        let mut routes = RouteManager::new(
            self.parts.route_table.clone(),
            link.luid,
            &upstream_ips,
            settings.idle_expiry,
            self.parts.log.with_profile(&name).with_source("routes"),
        );
        routes.set_guard(self.parts.guards.guard_for(&name));
        let statics: Vec<Ipv4Net> = self
            .inventory
            .lists
            .get(&name)
            .map(|list| list.networks().copied().collect())
            .unwrap_or_default();
        routes.set_statics(&statics);
        routes.reconcile(now);
        let installed = routes.snapshot().installed;
        let subnet = self
            .profile
            .as_ref()
            .expect("step 1 done")
            .facts
            .address
            .subnet();
        self.parts.active.insert(ActiveProfile {
            name,
            spec,
            settings: self.settings.clone(),
            subnet,
            upstream: self.upstream.clone(),
            link: Mutex::new(link),
            routes: Mutex::new(routes),
            handshakes: Mutex::new(HandshakePolicy::new(self.up_at)),
            health: Mutex::new(HandshakeHealth::Ok),
            traffic: Mutex::new(TrafficSeries::default()),
            held: Mutex::default(),
            since: now,
        });
        Ok(Outcome::Done(format!(
            "{installed} routes on LUID {:#x}",
            link.luid
        )))
    }

    /// 7: the upstream exchange bound to this adapter, and the profile's resolver; then the
    /// peer's first handshake awaited. The probe of a peer without a keepalive needs this step:
    /// the bound socket reaches the upstream only through step 6's routes.
    fn dns(&mut self) -> Result<Outcome, String> {
        let link = self.link.expect("step 4 done");
        let profile = self
            .parts
            .active
            .get(self.name.as_str())
            .expect("step 6 done");
        let exchange = self.parts.bound.bound(profile.spec.address, link.if_index);
        let servers: Vec<SocketAddr> = self.upstream.iter().map(|s| SocketAddr::V4(*s)).collect();
        let resolver = Arc::new(ProfileResolver::new(
            servers,
            exchange,
            self.parts.clock.clone(),
        ));
        self.parts
            .resolvers
            .insert(self.name.as_str(), resolver.clone());
        let handshake = self.first_handshake(profile.spec.keepalive, &resolver);
        Ok(Outcome::Done(format!(
            "upstream {} from {} through index {}; {handshake}",
            Self::join(&self.upstream),
            profile.spec.address,
            link.if_index
        )))
    }

    /// Waits up to `HandshakeWait::WAIT` for the peer's first handshake. A peer without a
    /// keepalive handshakes only when traffic comes, so it gets one probe first.
    fn first_handshake(&mut self, keepalive: u16, resolver: &ProfileResolver) -> String {
        let name = self.name.as_str();
        let clock = &*self.parts.clock;
        let wait = HandshakeWait::new(&*self.parts.tunnel, clock);
        let since = clock.now();
        let probe = keepalive == 0 && wait.current(name).is_none();
        if probe {
            resolver.probe(&self.probe_query());
        }
        let sent = if probe { "probe sent; " } else { "" };
        let Some(at) = wait.wait(name, since) else {
            // The alert, raised once the Connect is done, also logs it.
            self.missed_handshake = true;
            let secs = HandshakeWait::WAIT.as_secs();
            return format!("{sent}no handshake within {secs} s; connecting anyway");
        };
        resolver.handshake(at);
        let waited = clock.now().duration_since(since).unwrap_or_default();
        format!("{sent}first handshake after {:.1} s", waited.as_secs_f64())
    }

    /// The probe: the SOA of the profile's first listed domain, or of the root without one.
    fn probe_query(&self) -> Vec<u8> {
        let domain = self
            .inventory
            .lists
            .get(self.name.as_str())
            .and_then(|list| list.domains().next().map(ToString::to_string))
            .unwrap_or_else(|| ".".into());
        DnsQuery::build(Self::PROBE_ID, &domain, TYPE_SOA)
    }

    /// 8: the matcher with this profile; learned routes re-homed.
    fn matcher(&mut self) -> Result<Outcome, String> {
        let connected = self.parts.active.names();
        self.parts.rehome(&self.inventory.lists, &connected);
        let domains = self
            .inventory
            .lists
            .get(self.name.as_str())
            .map_or(0, |list| list.domains().count());
        Ok(Outcome::Done(format!("{domains} listed domains")))
    }

    /// 9: the NRPT rules synced to the union; the Windows cache flushed.
    fn nrpt(&mut self) -> Result<Outcome, String> {
        let connected = self.parts.active.names();
        self.parts
            .sync_nrpt(&self.inventory.lists, &connected)
            .map_err(|e| format!("writing the NRPT rules failed: {e}"))?;
        let rules = self.parts.nrpt.count().unwrap_or_default();
        Ok(Outcome::Done(format!("{rules} rules; cache flushed")))
    }

    /// 10: in `connected=`.
    fn state(&mut self) -> Result<Outcome, String> {
        let name = self.name.as_str();
        if self.inventory.state.add(name) {
            self.parts
                .write_state(&self.inventory.state)
                .map_err(|e| e.to_string())?;
        }
        Ok(Outcome::Done("in connected=".into()))
    }

    fn spec(
        name: &ProfileName,
        profile: &Profile,
        settings: &ProfileSettings,
        endpoint_host: String,
        endpoint: SocketAddrV4,
    ) -> TunnelSpec {
        let facts = &profile.facts;
        TunnelSpec {
            name: name.to_string(),
            adapter_guid: facts.guid,
            address: facts.address.address,
            mtu: settings.mtu.unwrap_or_else(|| facts.mtu_or_default()),
            private_key: profile.private_key.clone(),
            peer_public_key: facts.peer_public_key,
            preshared_key: profile.preshared_key.clone(),
            endpoint_host,
            endpoint,
            keepalive: facts.keepalive,
        }
    }

    fn join(servers: &[SocketAddrV4]) -> String {
        servers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

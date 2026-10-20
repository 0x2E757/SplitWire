//! Import previews and imports, by `ImportPreview` against the demo's profiles.

use std::time::Instant;

use super::DemoEngine;
use crate::ipc::{ConnectStep, ErrorCode, ProfileState, RefusedProblem, Response};
use crate::list::ListSet;
use crate::presentation::transport::demo::Scenario;
use crate::presentation::transport::demo::model::{DemoChart, DemoProfile};
use crate::presentation::transport::demo::reports::DemoReports;
use crate::profile::{ImportContext, ImportPreview, ImportRequest, Pin, WgConfParser};

impl DemoEngine {
    pub(super) fn preview(&self, name: &str, file_name: &str, conf_text: &str) -> ImportPreview {
        let known = DemoReports::known(&self.model);
        let lists = ListSet::load(self.model.lists());
        ImportPreview::build(
            &ImportRequest {
                name,
                file_name,
                conf_text,
            },
            &ImportContext {
                profiles: &known,
                lists: &lists,
                replaced: None,
                endpoint_addresses: &[],
                answers: &[],
            },
        )
    }

    pub(super) fn import(
        &mut self,
        name: &str,
        conf_text: &str,
        colour: Pin,
        replace: bool,
    ) -> Response {
        let preview = self.preview(name, name, conf_text);
        if preview.blocks() {
            return Self::error(
                ErrorCode::Refused(
                    preview
                        .problems()
                        .into_iter()
                        .map(RefusedProblem::Text)
                        .collect(),
                ),
                "the import was refused",
            );
        }
        let Some(conf) = WgConfParser::parse(conf_text).into_conf() else {
            return Self::refused("the .conf has a problem");
        };
        let template = DemoProfile {
            name: name.into(),
            colour,
            state: ProfileState::Off,
            since: None,
            uptime_secs: None,
            last_connected: None,
            handshake_age_secs: None,
            rx_rate: 0,
            tx_rate: 0,
            rx_total: 0,
            tx_total: 0,
            dns_rtt_ms: None,
            address: conf.address,
            mtu: conf.mtu.unwrap_or(crate::tunnel::TunnelSpec::DEFAULT_MTU),
            endpoint: conf.endpoint.clone(),
            endpoint_addresses: conf.endpoint.literal().into_iter().collect(),
            interface_key: String::new(),
            peer_key: crate::profile::Base64::encode(&conf.peer_public_key),
            dns: conf
                .dns
                .iter()
                .map(|ip| std::net::SocketAddrV4::new(*ip, 53))
                .collect(),
            keepalive: conf.keepalive,
            imported: Scenario::now(),
            list: String::new(),
            routes: Vec::new(),
            settings: crate::config::ProfileSettings::NEW_FILE.into(),
            attempt: None,
            chart: DemoChart {
                level: 8.0,
                seed: 11 + self.model.profiles.len() as u64,
            },
            connecting_since: None,
            traffic_from: 0,
        };
        let summary = if replace {
            let Some(profile) = self.model.profile_mut(name) else {
                return Self::unknown(name);
            };
            let reconnect = profile.connected();
            profile.address = template.address;
            profile.mtu = template.mtu;
            profile.endpoint = template.endpoint;
            profile.endpoint_addresses = template.endpoint_addresses;
            profile.peer_key = template.peer_key;
            profile.dns = template.dns;
            profile.keepalive = template.keepalive;
            profile.imported = template.imported;
            profile.colour = colour;
            if reconnect {
                profile.state = ProfileState::Connecting(ConnectStep::Peer);
                profile.connecting_since = Some(Instant::now());
            }
            DemoReports::summary(profile)
        } else {
            let summary = DemoReports::summary(&template);
            self.model.profiles.push(template);
            summary
        };
        self.bump_all();
        Response::Imported(Box::new(summary))
    }
}

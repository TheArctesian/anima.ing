//! Network probes, one per check type (CHK-001, CHK-007, CHK-012).
//!
//! A network failure is a [`ProbeResult`] with `ok: false`, never an `Err` or a panic; invert,
//! retries and status belong to the evaluation (CHK-008). Every probe resolves its host once and
//! connects only to an address the [`AddressPolicy`] allows, so DNS rebinding cannot reach a
//! refused address. No probe starts a process or a shell on the hub (CHK-012).

mod dns;
pub(in crate::fleet::checks) mod docker;
mod http;
mod icmp;
mod outcome;
pub(in crate::fleet::checks) mod rdap;
mod resolve;
pub(in crate::fleet::checks) mod snmp;
mod ssh;
pub(in crate::fleet::checks) mod systemd;
mod tailscale;
mod tcp;
mod tls;

use std::{collections::HashMap, net::IpAddr, sync::Arc};

use chrono::{DateTime, Utc};

pub use http::BODY_CAP;
pub use outcome::{Failure, Judgement, MESSAGE_MAX, ProbeResult, clean_message};
pub(super) use ssh::{Reach, observe_listing};

use crate::fleet::{
    checks::config::{CheckConfig, HttpSecrets},
    targets::AddressPolicy,
};

/// HTTP and DNS checks name their own target in the configuration and ignore the host.
#[derive(Clone, Debug)]
pub struct ProbeTarget {
    pub host: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Overrides {
    /// Keyed by lower-case host name.
    pub hosts: Arc<HashMap<String, Vec<IpAddr>>>,
    pub dns_port: Option<u16>,
}

#[derive(Clone, Debug)]
pub struct ProbeEnv {
    pub policy: Arc<AddressPolicy>,
    pub now: DateTime<Utc>,
    pub localapi: Option<String>,
    pub(crate) overrides: Overrides,
}

impl ProbeEnv {
    #[must_use]
    pub fn new(policy: AddressPolicy, now: DateTime<Utc>) -> Self {
        Self {
            policy: Arc::new(policy),
            now,
            localapi: None,
            overrides: Overrides::default(),
        }
    }

    #[must_use]
    pub fn with_localapi(self, localapi: Option<String>) -> Self {
        Self { localapi, ..self }
    }
}

pub async fn probe(
    target: &ProbeTarget,
    config: &CheckConfig,
    secrets: Option<&HttpSecrets>,
    env: &ProbeEnv,
) -> ProbeResult {
    if config.validate().is_err() {
        return ProbeResult::failed(&Failure::BadConfig);
    }
    within(config, run(target, config, secrets, env)).await
}

/// The only timer that decides the outcome, so the same case always gives the same message
/// (CHK-001).
pub(super) async fn within(
    config: &CheckConfig,
    attempt: impl Future<Output = ProbeResult>,
) -> ProbeResult {
    match tokio::time::timeout(config.timeout(), attempt).await {
        Ok(result) => result,
        Err(_) if matches!(config, CheckConfig::Icmp { .. }) => {
            ProbeResult::failed(&Failure::NoReply)
        }
        Err(_) => ProbeResult::failed(&Failure::TimedOut),
    }
}

async fn run(
    target: &ProbeTarget,
    config: &CheckConfig,
    secrets: Option<&HttpSecrets>,
    env: &ProbeEnv,
) -> ProbeResult {
    let host = target.host.as_str();
    match config {
        CheckConfig::Icmp { ip_version, .. } => {
            icmp::probe(host, *ip_version, config.timeout(), env).await
        }
        CheckConfig::Tcp { port, .. } => tcp::probe(host, *port, env).await,
        CheckConfig::Tls {
            port, server_name, ..
        } => tls::probe(host, *port, server_name.as_deref(), env).await,
        CheckConfig::Http(http) => http::probe(http, secrets, env).await,
        CheckConfig::Dns {
            name,
            record_type,
            resolver,
            expected,
        } => {
            let query = dns::DnsQuery {
                name,
                record_type: *record_type,
                resolver: *resolver,
                expected: expected.as_deref(),
            };
            dns::probe(&query, config.timeout(), env).await
        }
        CheckConfig::TailscalePing { target, .. } => {
            tailscale::probe(target.as_deref(), host, env.localapi.as_deref(), env).await
        }
        CheckConfig::Push | CheckConfig::Group => ProbeResult::failed(&Failure::NotProbed),
        CheckConfig::Docker { .. } | CheckConfig::Systemd { .. } => {
            ProbeResult::failed(&Failure::NeedsLink)
        }
        // The SNMP probe takes its secrets in `snmp::observe`; this one has none to send.
        CheckConfig::Snmp(_) => ProbeResult::failed(&Failure::SecretMissing),
        // The RDAP probe keeps its bootstrap file in `rdap::Rdap`, which this one has not.
        CheckConfig::DomainExpiry { .. } => ProbeResult::unjudged(&Failure::NeedsRdap),
    }
}

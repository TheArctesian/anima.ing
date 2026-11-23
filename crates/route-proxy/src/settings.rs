//! The proxy is off unless `ANIMA_ROUTE_PROXY_LISTEN` is set, so no deploy listens by accident.

use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    time::Duration,
};

use crate::limits::{Caps, Limits, Network, Timeouts};

const HUB_SOCKET: &str = "ANIMA_ROUTE_PROXY_HUB_SOCKET";
const LISTEN: &str = "ANIMA_ROUTE_PROXY_LISTEN";
const LISTEN_PLAIN: &str = "ANIMA_ROUTE_PROXY_LISTEN_PLAIN";
const DOOR: &str = "ANIMA_ROUTE_PROXY_DOOR";
const DOOR_ADDRESSES: &str = "ANIMA_ROUTE_PROXY_DOOR_ADDRESSES";
const CONNECTIONS: &str = "ANIMA_ROUTE_PROXY_CONNECTIONS";
const NEW_PER_SECOND: &str = "ANIMA_ROUTE_PROXY_NEW_PER_SECOND";
const BYTES_PER_DAY: &str = "ANIMA_ROUTE_PROXY_BYTES_PER_DAY";
const HANDSHAKE_SECONDS: &str = "ANIMA_ROUTE_PROXY_HANDSHAKE_SECONDS";
const HEAD_SECONDS: &str = "ANIMA_ROUTE_PROXY_HEAD_SECONDS";
const REQUEST_SECONDS: &str = "ANIMA_ROUTE_PROXY_REQUEST_SECONDS";
const IDLE_SECONDS: &str = "ANIMA_ROUTE_PROXY_IDLE_SECONDS";
const WRITE_SECONDS: &str = "ANIMA_ROUTE_PROXY_WRITE_SECONDS";
const OPEN_TOTAL: &str = "ANIMA_ROUTE_PROXY_OPEN_TOTAL";
const OPEN_PER_CLIENT: &str = "ANIMA_ROUTE_PROXY_OPEN_PER_CLIENT";
const OPEN_PLAIN: &str = "ANIMA_ROUTE_PROXY_OPEN_PLAIN";
const PRIVATE_ADDRESS: &str = "ANIMA_ROUTE_PROXY_PRIVATE_ADDRESS";
const PRIVATE_NETWORKS: &str = "ANIMA_ROUTE_PROXY_PRIVATE_NETWORKS";
const UDP_FLOWS: &str = "ANIMA_ROUTE_PROXY_UDP_FLOWS";
const UDP_IDLE_SECONDS: &str = "ANIMA_ROUTE_PROXY_UDP_IDLE_SECONDS";
const UDP_NEW_PER_SECOND: &str = "ANIMA_ROUTE_PROXY_UDP_NEW_PER_SECOND";
const UDP_FLOWS_PER_NETWORK: &str = "ANIMA_ROUTE_PROXY_UDP_FLOWS_PER_NETWORK";
const DAY_MS: u64 = 24 * 60 * 60 * 1000;
const PER_CLIENT: u32 = 8;
const PER_ROUTE: u32 = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub hub_socket: PathBuf,
    pub listen: SocketAddr,
    /// The plain HTTP listener for ACME and the redirect (80); `None`: not served.
    pub listen_plain: Option<SocketAddr>,
    /// The connections come from the door with a PROXY header. Never on a self-hosted hub.
    pub door: bool,
    /// The only peers whose PROXY header is read; empty closes each door connection (SF-327).
    pub door_addresses: Vec<IpAddr>,
    pub limits: Limits,
    pub timeouts: Timeouts,
    pub tls_caps: Caps,
    pub plain_caps: Caps,
    /// Where the private routes listen; `None`: no private route is served.
    pub private: Option<Private>,
    pub udp_flows: usize,
    pub udp_idle: Duration,
    /// The burst is twice this; it keeps false UDP sources from filling the flows (SF-327).
    pub udp_new_per_second: u32,
    /// The flows of each UDP stream from one source network (`limits::source_network`).
    pub udp_flows_per_network: usize,
}

/// A private route listens only on `address` and answers only `networks` (Section 16.7, SF-245).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Private {
    pub address: IpAddr,
    pub networks: Vec<Network>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SettingsError {
    #[error("{0} is not valid")]
    Invalid(&'static str),
    #[error("{HUB_SOCKET} is not set")]
    NoHub,
    #[error("{PRIVATE_ADDRESS} needs {PRIVATE_NETWORKS}")]
    NoPrivateNetworks,
}

/// The settings, or `None` when the proxy is off.
///
/// # Errors
/// [`SettingsError`] for a malformed or missing value.
pub fn from_env(env: &BTreeMap<String, String>) -> Result<Option<Settings>, SettingsError> {
    let Some(listen) = read::<SocketAddr>(env, LISTEN)? else {
        return Ok(None);
    };
    let hub_socket = env
        .get(HUB_SOCKET)
        .filter(|path| path.starts_with('/'))
        .map(PathBuf::from)
        .ok_or(SettingsError::NoHub)?;
    Ok(Some(Settings {
        hub_socket,
        listen,
        listen_plain: read(env, LISTEN_PLAIN)?,
        door: read::<bool>(env, DOOR)?.unwrap_or(false),
        door_addresses: addresses(env, DOOR_ADDRESSES)?,
        limits: Limits {
            connections: positive(env, CONNECTIONS)?.unwrap_or(PER_ROUTE),
            new_per_second: read(env, NEW_PER_SECOND)?.unwrap_or(20),
            bytes_per_period: read(env, BYTES_PER_DAY)?.unwrap_or(0),
            period_ms: DAY_MS,
        },
        timeouts: timeouts(env)?,
        tls_caps: Caps {
            total: positive(env, OPEN_TOTAL)?.unwrap_or(1024),
            per_client: positive(env, OPEN_PER_CLIENT)?.unwrap_or(PER_CLIENT),
        },
        plain_caps: Caps {
            total: positive(env, OPEN_PLAIN)?.unwrap_or(256),
            per_client: positive(env, OPEN_PER_CLIENT)?.unwrap_or(PER_CLIENT),
        },
        private: private(env)?,
        udp_flows: positive(env, UDP_FLOWS)?.unwrap_or(64),
        udp_idle: positive::<u64>(env, UDP_IDLE_SECONDS)?
            .map_or(Duration::from_secs(60), Duration::from_secs),
        udp_new_per_second: positive(env, UDP_NEW_PER_SECOND)?.unwrap_or(8),
        udp_flows_per_network: positive(env, UDP_FLOWS_PER_NETWORK)?.unwrap_or(4),
    }))
}

fn private(env: &BTreeMap<String, String>) -> Result<Option<Private>, SettingsError> {
    let Some(address) = read::<IpAddr>(env, PRIVATE_ADDRESS)? else {
        return Ok(None);
    };
    let networks = env.get(PRIVATE_NETWORKS).map_or(Ok(Vec::new()), |text| {
        text.split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| {
                part.parse()
                    .map_err(|()| SettingsError::Invalid(PRIVATE_NETWORKS))
            })
            .collect::<Result<Vec<Network>, _>>()
    })?;
    if networks.is_empty() {
        return Err(SettingsError::NoPrivateNetworks);
    }
    Ok(Some(Private { address, networks }))
}

fn timeouts(env: &BTreeMap<String, String>) -> Result<Timeouts, SettingsError> {
    let default = Timeouts::default();
    let seconds = |name, default: Duration| {
        positive::<u64>(env, name).map(|value| value.map_or(default, Duration::from_secs))
    };
    Ok(Timeouts {
        handshake: seconds(HANDSHAKE_SECONDS, default.handshake)?,
        head: seconds(HEAD_SECONDS, default.head)?,
        request: seconds(REQUEST_SECONDS, default.request)?,
        idle: seconds(IDLE_SECONDS, default.idle)?,
        write: seconds(WRITE_SECONDS, default.write)?,
    })
}

fn addresses(
    env: &BTreeMap<String, String>,
    name: &'static str,
) -> Result<Vec<IpAddr>, SettingsError> {
    env.get(name).map_or(Ok(Vec::new()), |text| {
        text.split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| part.parse().map_err(|_| SettingsError::Invalid(name)))
            .collect()
    })
}

fn positive<T: std::str::FromStr + Default + PartialEq>(
    env: &BTreeMap<String, String>,
    name: &'static str,
) -> Result<Option<T>, SettingsError> {
    match read::<T>(env, name)? {
        Some(value) if value == T::default() => Err(SettingsError::Invalid(name)),
        value => Ok(value),
    }
}

fn read<T: std::str::FromStr>(
    env: &BTreeMap<String, String>,
    name: &'static str,
) -> Result<Option<T>, SettingsError> {
    match env.get(name).map(String::as_str) {
        None | Some("") => Ok(None),
        Some(text) => text
            .parse()
            .map(Some)
            .map_err(|_| SettingsError::Invalid(name)),
    }
}

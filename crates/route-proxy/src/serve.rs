//! The state that each connection of the proxy shares (Section 12.3): the names and the streams
//! of the table of the hub, the limits of each name, the caps of each listener and the counts.
//! The listeners are in [`https`] (TLS for the names that serve, public and private) and
//! [`http`] (plain HTTP for the ACME challenge and the redirect), and the streams in
//! [`crate::streams`]. Each listener caps its connections in all and for each client network
//! before the TLS handshake, each step has a time limit, a request has one deadline for its head
//! and its body, and in door mode only the door's addresses may send a PROXY header
//! (SF-NEW-revroutelimits). A private route answers only on the private address and only the
//! nodes of the network of Anima (Section 16.7, SF-245). The log has only the name,
//! the bytes, the status and the time: never a path, a query, a header or a cookie (LOG-007,
//! SF-229). I/O edge.

pub mod http;
pub mod https;

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex, PoisonError, RwLock},
    time::{Duration, Instant},
};

use rustls::ServerConfig;
use tokio::{io::AsyncReadExt as _, net::TcpStream, time::timeout};
use uuid::Uuid;

use crate::{
    door_header,
    hub::Hub,
    limits::{Gate, Limiter, Timeouts},
    settings::{Private, Settings},
    streams::listeners::{Listeners, Places},
    wire::{Counts, Reach, StreamRoute},
};

/// One name that serves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub application: Uuid,
    pub reach: Reach,
    /// The one `https://` address of a redirect.
    pub redirect_to: Option<String>,
}

/// The state that each connection shares.
pub struct Proxy<H: Hub> {
    hub: H,
    /// Whether each connection comes from the door with a PROXY header.
    door: bool,
    /// The peers whose PROXY header is read in door mode.
    door_addresses: Vec<IpAddr>,
    private: Option<Private>,
    places: Places,
    tls: RwLock<Arc<ServerConfig>>,
    names: RwLock<HashMap<String, Entry>>,
    challenges: RwLock<HashMap<String, String>>,
    wanted_streams: Mutex<Vec<StreamRoute>>,
    listeners: Listeners,
    limiter: Mutex<Limiter>,
    timeouts: Timeouts,
    udp_flows: usize,
    udp_idle: Duration,
    tls_gate: Arc<Mutex<Gate>>,
    plain_gate: Arc<Mutex<Gate>>,
    stream_gate: Arc<Mutex<Gate>>,
    counts: Mutex<HashMap<String, Counts>>,
    started: Instant,
}

impl<H: Hub> Proxy<H> {
    /// A proxy with no name until the first [`Proxy::refresh`].
    ///
    /// # Errors
    /// When the TLS library has no safe default.
    pub fn new(hub: H, settings: &Settings) -> Result<Self, rustls::Error> {
        let (config, _) = crate::tls::server_config(&crate::wire::Table::default())?;
        Ok(Self {
            hub,
            door: settings.door,
            door_addresses: settings.door_addresses.clone(),
            private: settings.private.clone(),
            places: Places {
                public: settings.listen.ip(),
                private: settings.private.as_ref().map(|private| private.address),
                door: settings.door,
            },
            tls: RwLock::new(config),
            names: RwLock::default(),
            challenges: RwLock::default(),
            wanted_streams: Mutex::default(),
            listeners: Listeners::default(),
            limiter: Mutex::new(Limiter::new(settings.limits)),
            timeouts: settings.timeouts,
            udp_flows: settings.udp_flows,
            udp_idle: settings.udp_idle,
            tls_gate: Arc::new(Mutex::new(Gate::new(settings.tls_caps))),
            plain_gate: Arc::new(Mutex::new(Gate::new(settings.plain_caps))),
            stream_gate: Arc::new(Mutex::new(Gate::new(settings.tls_caps))),
            counts: Mutex::default(),
            started: Instant::now(),
        })
    }

    /// Reads the table of the hub again; returns the names that serve and those left out.
    ///
    /// # Errors
    /// When the hub does not answer, or the TLS library refuses.
    pub async fn refresh(&self) -> Result<(usize, Vec<String>), Box<dyn std::error::Error>> {
        let table = self.hub.table().await?;
        let (config, left_out) = crate::tls::server_config(&table)?;
        let names: HashMap<String, Entry> = table
            .names
            .iter()
            .filter(|named| !left_out.contains(&named.name))
            .map(|named| {
                let entry = Entry {
                    application: named.application,
                    reach: named.reach,
                    redirect_to: named.redirect_to.clone(),
                };
                (named.name.clone(), entry)
            })
            .collect();
        let challenges = table
            .challenges
            .into_iter()
            .map(|challenge| (challenge.token, challenge.key_authorization))
            .collect();
        let serving = names.len();
        *self.tls.write().unwrap_or_else(PoisonError::into_inner) = config;
        *self.names.write().unwrap_or_else(PoisonError::into_inner) = names;
        *self
            .challenges
            .write()
            .unwrap_or_else(PoisonError::into_inner) = challenges;
        *self
            .wanted_streams
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = table.streams;
        Ok((serving, left_out))
    }

    /// Opens and stops the listeners of the streams as the last [`Proxy::refresh`] read them.
    pub async fn keep_streams(self: &Arc<Self>) {
        let wanted = self
            .wanted_streams
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        self.listeners.keep(self, self.places, wanted).await;
    }

    /// Gives the hub what was counted since the last report.
    pub async fn report(&self) {
        let counts =
            std::mem::take(&mut *self.counts.lock().unwrap_or_else(PoisonError::into_inner));
        for (name, counts) in counts {
            if let Err(error) = self.hub.count(&name, counts).await {
                tracing::warn!(error = %error, "the hub did not take the counts");
            }
        }
    }

    pub(crate) const fn hub(&self) -> &H {
        &self.hub
    }

    pub(crate) const fn timeouts(&self) -> Timeouts {
        self.timeouts
    }

    pub(crate) const fn stream_gate(&self) -> &Arc<Mutex<Gate>> {
        &self.stream_gate
    }

    pub(crate) const fn udp_flows(&self) -> usize {
        self.udp_flows
    }

    pub(crate) const fn udp_idle(&self) -> Duration {
        self.udp_idle
    }

    /// The name that serves as `name`.
    pub(crate) fn entry(&self, name: &str) -> Option<Entry> {
        let names = self.names.read().unwrap_or_else(PoisonError::into_inner);
        names.get(name).cloned()
    }

    /// Whether `client` may reach a route of `reach`: each client for a public one, only a node
    /// of the network of Anima for a private one.
    pub(crate) fn reaches(&self, reach: Reach, client: IpAddr) -> bool {
        match (reach, &self.private) {
            (Reach::Public, _) => true,
            (Reach::Private, Some(private)) => private
                .networks
                .iter()
                .any(|network| network.contains(client)),
            (Reach::Private, None) => false,
        }
    }

    /// Admits one new connection to `name`; a refusal is counted.
    pub(crate) fn admit(&self, name: &str) -> bool {
        let admitted = self
            .limiter
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .admit(name, self.now_ms());
        if admitted.is_err() {
            self.add(name, |counts| counts.refused += 1);
        }
        admitted.is_ok()
    }

    /// A connection to `name` ended.
    pub(crate) fn release(&self, name: &str) {
        self.limiter
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .release(name);
    }

    /// Counts one request or connection of `name` and its bytes in its period.
    pub(crate) fn moved(&self, name: &str, bytes_in: u64, bytes_out: u64) {
        self.add(name, |counts| {
            counts.requests += 1;
            counts.bytes_in += bytes_in;
            counts.bytes_out += bytes_out;
        });
        self.limiter
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .add_bytes(name, bytes_in + bytes_out, self.now_ms());
    }

    fn now_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn add(&self, name: &str, change: impl FnOnce(&mut Counts)) {
        let mut counts = self.counts.lock().unwrap_or_else(PoisonError::into_inner);
        change(counts.entry(name.to_owned()).or_default());
    }

    /// The client of a connection: from the PROXY header of the door within the handshake
    /// time, or the peer; `None` closes the connection. In door mode a peer that is not an
    /// address of the door is closed before a byte is read.
    async fn client(&self, stream: &mut TcpStream, peer: IpAddr) -> Option<IpAddr> {
        if !self.door {
            return Some(peer);
        }
        if !door_header::from_door(peer, &self.door_addresses) {
            return None;
        }
        timeout(self.timeouts.handshake, client_of_door(stream))
            .await
            .ok()
            .flatten()
    }
}

/// The client address of the PROXY header of the door; `None` closes the connection.
async fn client_of_door(stream: &mut TcpStream) -> Option<IpAddr> {
    let mut line = Vec::with_capacity(door_header::HEADER_MAX);
    while line.len() < door_header::HEADER_MAX {
        let byte = stream.read_u8().await.ok()?;
        if byte == b'\n' {
            let text = std::str::from_utf8(&line).ok()?;
            return door_header::client(text.strip_suffix('\r')?);
        }
        line.push(byte);
    }
    None
}

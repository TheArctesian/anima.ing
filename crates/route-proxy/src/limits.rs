//! The limits of the proxy (Section 12.7, SF-327). The operator decides the numbers with the
//! prices (OD-HOST-40). Time is a parameter, in milliseconds from any fixed start. No I/O.

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv6Addr},
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeouts {
    /// From the accept to the end of the TLS handshake, the PROXY header of the door included.
    pub handshake: Duration,
    pub head: Duration,
    /// For head and body together, so a client that trickles bytes cannot hold a stream longer.
    pub request: Duration,
    pub idle: Duration,
    /// For each write to the client, so a slow reader cannot hold a stream of the link (SF-327).
    pub write: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            handshake: Duration::from_secs(10),
            head: Duration::from_secs(10),
            request: Duration::from_secs(60),
            idle: Duration::from_secs(300),
            write: Duration::from_secs(30),
        }
    }
}

/// The connections that one listener keeps open before it knows a name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caps {
    pub total: u32,
    /// Below `ANIMA_ROUTES_STREAMS_PER_LINK` (32), so one client cannot hold each stream of a device.
    pub per_client: u32,
}

/// The /64 of an IPv6 address, which one host gets for itself, or the whole IPv4 address.
#[must_use]
pub fn client_key(client: IpAddr) -> IpAddr {
    match client.to_canonical() {
        IpAddr::V6(address) => {
            let network = u128::from(address) & !((1_u128 << 64) - 1);
            IpAddr::V6(Ipv6Addr::from(network))
        }
        v4 @ IpAddr::V4(_) => v4,
    }
}

/// The /64 of IPv6 or the /24 of IPv4, so false sources in one network take few flows (SF-327).
#[must_use]
pub fn source_network(source: IpAddr) -> IpAddr {
    match source.to_canonical() {
        IpAddr::V4(address) => IpAddr::V4((u32::from(address) & !0xff).into()),
        v6 @ IpAddr::V6(_) => client_key(v6),
    }
}

/// `address/prefix`: the Anima nodes that may reach a private route (Section 16.7, SF-245).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Network {
    address: IpAddr,
    prefix: u8,
}

impl std::str::FromStr for Network {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, ()> {
        let (address, prefix) = text.split_once('/').ok_or(())?;
        let address: IpAddr = address.parse().map_err(|_| ())?;
        let prefix: u8 = prefix.parse().map_err(|_| ())?;
        let most = if address.is_ipv4() { 32 } else { 128 };
        (prefix <= most)
            .then_some(Self { address, prefix })
            .ok_or(())
    }
}

impl Network {
    #[must_use]
    pub fn contains(&self, address: IpAddr) -> bool {
        match (self.address, address.to_canonical()) {
            (IpAddr::V4(network), IpAddr::V4(address)) => {
                let mask = u32::MAX
                    .checked_shl(32 - u32::from(self.prefix))
                    .unwrap_or(0);
                u32::from(network) & mask == u32::from(address) & mask
            }
            (IpAddr::V6(network), IpAddr::V6(address)) => {
                let mask = u128::MAX
                    .checked_shl(128 - u32::from(self.prefix))
                    .unwrap_or(0);
                u128::from(network) & mask == u128::from(address) & mask
            }
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct Gate {
    caps: Caps,
    total: u32,
    clients: HashMap<IpAddr, u32>,
}

impl Gate {
    #[must_use]
    pub fn new(caps: Caps) -> Self {
        Self {
            caps,
            total: 0,
            clients: HashMap::new(),
        }
    }

    pub fn enter(&mut self) -> bool {
        if self.total >= self.caps.total {
            return false;
        }
        self.total += 1;
        true
    }

    pub fn leave(&mut self) {
        self.total = self.total.saturating_sub(1);
    }

    pub fn enter_client(&mut self, client: IpAddr) -> bool {
        let open = self.clients.entry(client_key(client)).or_default();
        if *open >= self.caps.per_client {
            return false;
        }
        *open += 1;
        true
    }

    pub fn leave_client(&mut self, client: IpAddr) {
        let key = client_key(client);
        if let Some(open) = self.clients.get_mut(&key) {
            *open = open.saturating_sub(1);
            if *open == 0 {
                self.clients.remove(&key);
            }
        }
    }
}

/// One place under a [`Gate`] until it is dropped: in all, and for its client once known.
pub struct Pass {
    gate: Arc<Mutex<Gate>>,
    client: Option<IpAddr>,
}

impl Pass {
    #[must_use]
    pub fn enter(gate: &Arc<Mutex<Gate>>) -> Option<Self> {
        let entered = gate.lock().unwrap_or_else(PoisonError::into_inner).enter();
        entered.then(|| Self {
            gate: Arc::clone(gate),
            client: None,
        })
    }

    pub fn admit(&mut self, client: IpAddr) -> bool {
        let mut gate = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
        let admitted = gate.enter_client(client);
        if admitted {
            self.client = Some(client);
        }
        admitted
    }
}

impl Drop for Pass {
    fn drop(&mut self) {
        let mut gate = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(client) = self.client {
            gate.leave_client(client);
        }
        gate.leave();
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Bucket {
    per_second: u32,
    burst: u32,
    tokens_milli: u64,
    refilled_ms: u64,
}

impl Bucket {
    #[must_use]
    pub fn full(per_second: u32, burst: u32, now_ms: u64) -> Self {
        Self {
            per_second,
            burst,
            tokens_milli: u64::from(burst) * 1000,
            refilled_ms: now_ms,
        }
    }

    pub fn take(&mut self, now_ms: u64) -> bool {
        let elapsed = now_ms.saturating_sub(self.refilled_ms);
        let added = elapsed.saturating_mul(u64::from(self.per_second));
        self.tokens_milli = self
            .tokens_milli
            .saturating_add(added)
            .min(u64::from(self.burst) * 1000);
        self.refilled_ms = now_ms;
        if self.tokens_milli < 1000 {
            return false;
        }
        self.tokens_milli -= 1000;
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Below the streams of one hub link, so one route's clients cannot hold its device (SF-327).
    pub connections: u32,
    /// New connections each second, with a burst of the same number.
    pub new_per_second: u32,
    /// Bytes in both directions in each period; 0: no limit.
    pub bytes_per_period: u64,
    pub period_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Connections,
    Rate,
    Bytes,
}

#[derive(Clone, Copy, Debug, Default)]
struct State {
    open: u32,
    arrivals: Bucket,
    period_start_ms: u64,
    bytes: u64,
    started: bool,
}

#[derive(Debug)]
pub struct Limiter {
    limits: Limits,
    names: HashMap<String, State>,
}

impl Limiter {
    #[must_use]
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            names: HashMap::new(),
        }
    }

    /// # Errors
    /// The [`Refusal`] of the first limit that is reached.
    pub fn admit(&mut self, name: &str, now_ms: u64) -> Result<(), Refusal> {
        let limits = self.limits;
        let state = self.state(name, now_ms);
        if state.open >= limits.connections {
            return Err(Refusal::Connections);
        }
        if limits.bytes_per_period > 0 && state.bytes >= limits.bytes_per_period {
            return Err(Refusal::Bytes);
        }
        if !state.arrivals.take(now_ms) {
            return Err(Refusal::Rate);
        }
        state.open += 1;
        Ok(())
    }

    pub fn release(&mut self, name: &str) {
        if let Some(state) = self.names.get_mut(name) {
            state.open = state.open.saturating_sub(1);
        }
    }

    /// Adds `bytes` to the period of `name`; whether the name is still under its limit.
    pub fn add_bytes(&mut self, name: &str, bytes: u64, now_ms: u64) -> bool {
        let limit = self.limits.bytes_per_period;
        let state = self.state(name, now_ms);
        state.bytes = state.bytes.saturating_add(bytes);
        limit == 0 || state.bytes < limit
    }

    fn state(&mut self, name: &str, now_ms: u64) -> &mut State {
        let limits = self.limits;
        let state = self.names.entry(name.to_owned()).or_default();
        if !state.started {
            *state = State {
                arrivals: Bucket::full(limits.new_per_second, limits.new_per_second, now_ms),
                period_start_ms: now_ms,
                started: true,
                ..State::default()
            };
        }
        if now_ms.saturating_sub(state.period_start_ms) >= limits.period_ms {
            state.period_start_ms = now_ms;
            state.bytes = 0;
        }
        state
    }
}

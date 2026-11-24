//! The loads of the copies of the node (K11 Section 11.1, FIT-020 to FIT-025; task L1-16): room for a load, the
//! unload of idle copies, the pin and its return, and the keep files. The rules are in
//! `daemon_core::model_work::memory`; this module does the I/O.
//!
//! One lock holds all of it, so the node does one load at a time on all its memories (stricter than one load at a
//! time on each memory). A `model_load` that waits sets `waiting`, and a return that runs gives way.

mod kept;
pub mod router;
mod steps;
mod tick;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::folders::Folders;
use daemon_core::model_work::memory::keep::Kept;
use daemon_core::model_work::memory::room::Usable;
use daemon_core::model_work::memory::{Held, Memory, OnServer, reserved_on};
use daemon_core::model_work::order::Parameters;
use daemon_core::model_work::settings::ServerSettings;
use serde_json::{Value, json};
use uuid::Uuid;

pub use router::Router;

use super::supervise::{self, ServerState};
pub use steps::Ask;

/// The most entries of `models` of a server, and of `displaced` of a memory.
const MAX_LISTED: usize = 16;

/// A copy that holds a reservation, on one server.
#[derive(Clone, Debug)]
pub struct OnNode {
    pub held: Held,
    pub server: Uuid,
    pub model: PathBuf,
    pub projector: Option<PathBuf>,
    pub settings: ServerSettings,
    /// The last `n_decode_total` of the copy: a change is a request.
    pub decodes: u64,
}

pub struct Loads {
    copies: BTreeMap<Uuid, OnNode>,
    kept: BTreeMap<Uuid, Kept>,
    busy_at: BTreeMap<Memory, u64>,
    folders: Folders,
}

/// The loads of the node, shared by the claims and the 15-second check.
pub struct Shared {
    pub loads: tokio::sync::Mutex<Loads>,
    /// A `model_load` waits for the lock.
    pub waiting: AtomicBool,
    /// The switch is on and the device policy permits model work (Section 3): a return may run.
    pub permitted: AtomicBool,
}

/// The servers of the node that the daemon can reach now, by server.
pub type Routers = BTreeMap<Uuid, Router>;

/// The node at the time of one step: its servers, its usable memory and the clock.
pub struct At<'a> {
    pub routers: &'a Routers,
    pub usable: &'a Usable,
    pub now: u64,
}

/// The clock of the loads: Unix seconds, so that a keep file holds its times across a restart.
#[must_use]
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// The usable memory of the node: the main memory times `share` (RTC-003).
// TODO(L1-5): the GPUs of the inventory, and the share of the device policy.
#[must_use]
pub fn usable(share: f64) -> Usable {
    let total = main_memory().unwrap_or(0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a share from 0 to 1 of a memory size; the product is not negative and not larger than the size"
    )]
    let bytes = (total as f64 * share.clamp(0.0, 1.0)) as u64;
    BTreeMap::from([(Memory::Main, bytes)])
}

#[cfg(target_os = "linux")]
fn main_memory() -> Option<u64> {
    let text = std::fs::read("/proc/meminfo").ok()?;
    daemon_core::readings::linux::memory(&text).map(|memory| memory.total_bytes)
}

#[cfg(not(target_os = "linux"))]
fn main_memory() -> Option<u64> {
    let output = std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    String::from_utf8(output.stdout).ok()?.trim().parse().ok()
}

impl Shared {
    #[must_use]
    pub fn open(folders: Folders) -> Self {
        Self {
            loads: tokio::sync::Mutex::new(Loads::open(folders)),
            waiting: AtomicBool::new(false),
            permitted: AtomicBool::new(false),
        }
    }

    /// The lock for a `model_load`: a return that runs gives way.
    ///
    /// # Errors
    /// `memory_busy` after [`router::WAIT_SECONDS`].
    pub async fn for_load(&self) -> Result<tokio::sync::MutexGuard<'_, Loads>, Cause> {
        self.waiting.store(true, Ordering::SeqCst);
        let wait = Duration::from_secs(router::WAIT_SECONDS);
        let lock = tokio::time::timeout(wait, self.loads.lock()).await;
        self.waiting.store(false, Ordering::SeqCst);
        lock.map_err(|_| Cause::MemoryBusy)
    }

    /// `model_load`, `model_unload` and `model_keep`; `None` for an other order.
    pub async fn run(
        &self,
        parameters: &Parameters,
        servers: &supervise::Shared,
    ) -> Option<Result<(), Cause>> {
        let memory = usable(1.0);
        let done = match parameters {
            Parameters::Load {
                server,
                copy,
                settings,
                reserve_bytes,
                keep,
            } => {
                let routers = routers(servers).await;
                let ask = Ask {
                    server: *server,
                    copy: *copy,
                    settings,
                    reserve_bytes: *reserve_bytes,
                    keep: *keep,
                };
                match self.for_load().await {
                    Ok(mut loads) => loads.load(&at(&routers, &memory), &ask).await,
                    Err(cause) => Err(cause),
                }
            }
            Parameters::Unload { server, copy } => {
                let routers = routers(servers).await;
                let mut loads = self.loads.lock().await;
                loads.unload(&at(&routers, &memory), *server, *copy).await
            }
            Parameters::Keep { copy, keep } => self.loads.lock().await.keep(*copy, *keep),
            _ => return None,
        };
        Some(done)
    }

    /// The 15-second work; nothing while a `model_load` holds the lock.
    pub async fn tick(&self, servers: &supervise::Shared) {
        let Ok(mut loads) = self.loads.try_lock() else {
            return;
        };
        let routers = routers(servers).await;
        let permitted = self.permitted.load(Ordering::SeqCst);
        let memory = usable(1.0);
        loads
            .tick(&at(&routers, &memory), permitted, &self.waiting)
            .await;
    }

    /// The members `models` of each server and `memory` (Section 11.1); `None` when the lock does not come in a
    /// few seconds.
    pub async fn report(
        &self,
        usable: &Usable,
    ) -> Option<(BTreeMap<Uuid, Vec<Value>>, Vec<Value>)> {
        let loads = tokio::time::timeout(Duration::from_secs(5), self.loads.lock())
            .await
            .ok()?;
        Some((loads.models(now()), loads.memory(usable)))
    }
}

fn at<'a>(routers: &'a Routers, usable: &'a Usable) -> At<'a> {
    At {
        routers,
        usable,
        now: now(),
    }
}

/// The servers that are `ready` in router mode, as the daemon reaches them now; the lock of the servers is held
/// only to read them.
pub async fn routers(servers: &supervise::Shared) -> Routers {
    let servers = servers.lock().await;
    servers
        .map
        .iter()
        .filter(|(_, server)| {
            server.state == ServerState::Ready && server.runtime == "llama_server"
        })
        .filter_map(|(id, server)| {
            let (running, port) = server.running.as_ref()?;
            Some((*id, Router::new(*port, running.key.clone())?))
        })
        .collect()
}

impl Loads {
    fn open(folders: Folders) -> Self {
        let kept = kept::open(&folders.state);
        Self {
            copies: BTreeMap::new(),
            kept,
            busy_at: BTreeMap::new(),
            folders,
        }
    }

    fn held(&self) -> Vec<Held> {
        self.copies.values().map(|copy| copy.held.clone()).collect()
    }

    fn models(&self, now: u64) -> BTreeMap<Uuid, Vec<Value>> {
        let mut by_server: BTreeMap<Uuid, Vec<Value>> = BTreeMap::new();
        for copy in self.copies.values() {
            let held = &copy.held;
            let ago = match held.state {
                OnServer::Active | OnServer::Loading => 0,
                _ => now.saturating_sub(held.used_at) / 60 * 60,
            };
            let list = by_server.entry(copy.server).or_default();
            if list.len() < MAX_LISTED {
                list.push(json!({
                    "copy": held.copy, "state": held.state.as_str(), "reservedBytes": held.reserved,
                    "idleSeconds": held.keep.idle_seconds, "pinned": held.keep.pinned, "usedSecondsAgo": ago,
                }));
            }
        }
        by_server
    }

    fn memory(&self, usable: &Usable) -> Vec<Value> {
        let held = self.held();
        let displaced: Vec<Uuid> = self
            .kept
            .iter()
            .filter(|(_, kept)| kept.displaced && kept.keep.pinned)
            .map(|(copy, _)| *copy)
            .take(MAX_LISTED)
            .collect();
        usable
            .iter()
            .map(|(memory, bytes)| {
                // TODO(L1-5): a displaced copy counts against the memories of its load; with no GPU, that is main.
                let waiting = if *memory == Memory::Main {
                    displaced.clone()
                } else {
                    Vec::new()
                };
                json!({
                    "memory": memory.word(), "usableBytes": bytes,
                    "reservedBytes": reserved_on(&held, *memory), "displaced": waiting,
                })
            })
            .collect()
    }
}

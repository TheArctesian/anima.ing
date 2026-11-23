//! Model work in the running daemon (K11, SF-259): the node of this installation, its claims at
//! the rhythm of the switch at the machine (K10 Section 4), and the 15-second check of its servers.
//! Both loops are tasks of the async runtime, never of `spawn_blocking`, so a server that they start
//! keeps the thread that started it (SF-294).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use daemon_core::model_work::folders::{self, Folders, Installation};
use daemon_core::model_work::found;
use daemon_core::wire::{WorkArchitecture, WorkOs};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval};
use uuid::Uuid;

use super::thread_sync::Target;
use crate::config::home::Home;
use crate::config::origin::Origin;
use crate::hub::HubClient;
use crate::model::cycle::{self, Node};
use crate::model::install::{self, DEFAULT_RESERVE};
use crate::model::machine_files::{self, MachineFileError};
use crate::model::server::PORTS;
use crate::model::supervise::{Servers, Shared};
use crate::report;
use crate::token_store::TokenStore;
use crate::work::switch;

/// The time between two claims while the switch is on (K10 Section 4).
pub const CLAIM_ON: Duration = Duration::from_secs(10);

/// The time between two claims while the switch is off (K10 Section 4).
pub const CLAIM_OFF: Duration = Duration::from_hours(1);

/// The time between two checks of the servers (CAG-005, K11 Section 9).
pub const SERVERS_EVERY: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("this system or processor takes no model work")]
    Machine,
    #[error("no folders for model work: set HOME")]
    NoFolders,
    #[error("{0}")]
    Files(#[from] MachineFileError),
    #[error("the daemon program: {0}")]
    Program(std::io::Error),
}

/// Where the claims go: the client, the origin that the token must name, and the store whose
/// token is read again before each claim, so a renewal of the pusher is used.
pub struct Hub {
    pub client: Arc<HubClient>,
    pub origin: Origin,
    pub tokens: TokenStore,
}

/// When the daemon claims (K10 Section 4): at its start, each tick while the switch is on, and
/// while it is off at the first tick after a change and then once in each hour.
#[derive(Debug, Default)]
pub struct Rhythm {
    last: Option<(std::time::Instant, bool)>,
}

impl Rhythm {
    #[must_use]
    pub fn due(&self, on: bool, now: std::time::Instant) -> bool {
        self.last.is_none_or(|(at, was_on)| {
            on || was_on || now.saturating_duration_since(at) >= CLAIM_OFF
        })
    }

    pub const fn claimed(&mut self, on: bool, now: std::time::Instant) {
        self.last = Some((now, on));
    }
}

/// The node of this installation: the system unit when the daemon folder is the one of the
/// system unit, else the unit of the user (K11 Section 10).
///
/// # Errors
/// [`NodeError`] when this machine cannot do model work, or a file of model work does not read.
pub fn node(
    home: &Home,
    device: Uuid,
    env: impl Fn(&str) -> Option<String>,
) -> Result<Node, NodeError> {
    let (os, architecture) = machine().ok_or(NodeError::Machine)?;
    let installation = match os {
        WorkOs::Macos => Installation::MacUser,
        WorkOs::Linux if home.dir() == crate::units::plan::SYSTEM_HOME => Installation::LinuxSystem,
        WorkOs::Linux => Installation::LinuxUser,
    };
    let user_home = env("HOME").map(PathBuf::from);
    Ok(Node {
        device,
        os,
        architecture,
        folders: folders::layout(installation, &env).ok_or(NodeError::NoFolders)?,
        sources: machine_files::sources(home.dir())?,
        settings: machine_files::settings(home.dir())?,
        servers: Shared::default(),
        switch_on: false,
        started: std::time::Instant::now(),
        reserve: DEFAULT_RESERVE,
        launcher: std::env::current_exe().map_err(NodeError::Program)?,
        ports: PORTS,
        tool_folders: found::tool_folders(installation, user_home.as_deref()),
    })
}

const fn machine() -> Option<(WorkOs, WorkArchitecture)> {
    let os = if cfg!(target_os = "macos") {
        WorkOs::Macos
    } else if cfg!(target_os = "linux") {
        WorkOs::Linux
    } else {
        return None;
    };
    let architecture = if cfg!(target_arch = "x86_64") {
        WorkArchitecture::X86_64
    } else if cfg!(target_arch = "aarch64") {
        WorkArchitecture::Aarch64
    } else {
        return None;
    };
    Some((os, architecture))
}

/// The model work of the daemon in `home`; none for the daemon of a hosted hub, and none, with
/// the reason in the log, when its node does not build.
#[must_use]
pub(crate) fn begin(
    home: &Home,
    hosted_hub: bool,
    device: Uuid,
    target: Target,
) -> Option<ModelWork> {
    if hosted_hub {
        return None;
    }
    match node(home, device, |name| std::env::var(name).ok()) {
        Ok(node) => {
            let hub = Hub {
                client: target.hub,
                origin: target.origin,
                tokens: target.tokens,
            };
            Some(ModelWork::start(node, hub, home.clone()))
        }
        Err(error) => {
            report::model_work_off(&error);
            None
        }
    }
}

/// Ends the model work that [`begin`] started.
pub async fn end(model: Option<ModelWork>) {
    if let Some(model) = model {
        model.end().await;
    }
}

/// The two loops of model work, until [`ModelWork::end`].
pub struct ModelWork {
    halt: watch::Sender<bool>,
    claims: JoinHandle<()>,
}

impl ModelWork {
    /// Ends each server of an earlier run and each install that it left (SF-317), then starts
    /// the loops.
    #[must_use]
    pub fn start(node: Node, hub: Hub, home: Home) -> Self {
        match node.sweep() {
            Ok(ended) => report::servers_swept(ended),
            Err(error) => report::model_folder_failed(&error),
        }
        match install::clear_unpacks(&node.folders.runtimes) {
            Ok(cleared) => report::unpacks_cleared(cleared),
            Err(error) => report::model_folder_failed(&error),
        }
        let (halt, halted) = watch::channel(false);
        let claims = tokio::spawn(claims(node, hub, home, halted));
        Self { halt, claims }
    }

    /// Stops the loops, then each server: SIGTERM, a grace period, then the end of its tree
    /// (R-340).
    pub async fn end(self) {
        let _ = self.halt.send(true);
        if let Err(error) = self.claims.await {
            report::model_work_ended(&error);
        }
    }
}

async fn claims(mut node: Node, hub: Hub, home: Home, mut halted: watch::Receiver<bool>) {
    let check = tokio::spawn(check(
        Arc::clone(&node.servers),
        node.folders.clone(),
        node.started,
        halted.clone(),
    ));
    let mut ticks = interval(CLAIM_ON);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut rhythm = Rhythm::default();
    loop {
        tokio::select! {
            _ = halted.changed() => break,
            _ = ticks.tick() => {}
        }
        let on = switch_on(&home);
        let now = std::time::Instant::now();
        if !rhythm.due(on, now) {
            continue;
        }
        rhythm.claimed(on, now);
        node.switch_on = on;
        // A halt drops an item that runs: the end of a pull or a start ends its process.
        tokio::select! {
            _ = halted.changed() => break,
            () = claim(&mut node, &hub) => {}
        }
    }
    let _ = check.await;
    node.servers.lock().await.stop_all().await;
}

/// A switch file that does not read is off, and the log says why.
fn switch_on(home: &Home) -> bool {
    switch::is_on(home.dir()).unwrap_or_else(|error| {
        report::switch_unreadable(&error);
        false
    })
}

async fn claim(node: &mut Node, hub: &Hub) {
    let tokens = hub.tokens.clone();
    let credential = match tokio::task::spawn_blocking(move || tokens.load()).await {
        Ok(Ok(Some(credential))) if credential.hub == hub.origin => credential,
        // The pusher reports a token that does not read or that another hub issued.
        _ => return,
    };
    if let Err(cause) = node.cycle(&hub.client, &credential.token).await {
        report::claim_failed(cause);
    }
}

async fn check(
    servers: Shared,
    folders: Folders,
    started: std::time::Instant,
    mut halted: watch::Receiver<bool>,
) {
    let mut ticks = interval(SERVERS_EVERY);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = halted.changed() => return,
            _ = ticks.tick() => {}
        }
        let now = started.elapsed().as_secs();
        let mut servers: tokio::sync::MutexGuard<'_, Servers> = servers.lock().await;
        servers
            .cycle(now, |id| folders.server(id).join(cycle::TEMPORARY))
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn k10_4_claims_each_tick_while_on_and_once_an_hour_while_off() {
        let start = std::time::Instant::now();
        let mut rhythm = Rhythm::default();
        assert!(rhythm.due(false, start), "one claim at the start");
        rhythm.claimed(false, start);
        assert!(!rhythm.due(false, start + CLAIM_ON));
        assert!(!rhythm.due(false, start + Duration::from_secs(3_590)));
        assert!(rhythm.due(false, start + CLAIM_OFF));
        assert!(rhythm.due(true, start + CLAIM_ON), "on at once");
        rhythm.claimed(true, start + CLAIM_ON);
        assert!(rhythm.due(true, start + CLAIM_ON * 2));
        assert!(
            rhythm.due(false, start + CLAIM_ON * 2),
            "the hub learns at once that the switch went off"
        );
        rhythm.claimed(false, start + CLAIM_ON * 2);
        assert!(!rhythm.due(false, start + CLAIM_ON * 3));
    }
}

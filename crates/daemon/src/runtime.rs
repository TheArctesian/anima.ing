//! `anima-daemon run`: sampling, probing, usage reading and pushing are separate tasks, so a slow one never delays sampling (MET-014, K8 Section 7).

mod agent_usage;
mod link_wait;
#[cfg(target_os = "linux")]
mod managed_code;
#[cfg(target_os = "linux")]
mod managed_transcripts;
#[cfg(unix)]
pub mod old_origin;
#[cfg(target_os = "linux")]
pub(crate) mod owner_socket;
mod sampling;
#[cfg(target_os = "linux")]
mod source_work;
#[cfg(unix)]
pub(crate) mod thread_content;
#[cfg(unix)]
pub(crate) mod thread_sync;
mod token_wait;
#[cfg(target_os = "linux")]
mod workspaces;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep_until};
use uuid::Uuid;

use std::path::PathBuf;

use crate::config::home::{Home, HomeError};
use crate::hub::{HubClient, HubError, TIMEOUT};
use crate::keys::{keeper::Keeper, store::KeyStore};
use crate::push::pacing::RHYTHM;
use crate::push::{Event, Pusher};
use crate::readers::Platform;
use crate::report;
use crate::token_store::{Credential, TokenStore, TokenStoreError};
use agent_usage::{count, counting};
use link_wait::LinkWatch;

/// One enrolled hub destination, shared by independent publishing and command tasks.
#[cfg(unix)]
#[derive(Clone)]
pub(crate) struct HubTarget {
    pub(crate) hub: Arc<HubClient>,
    pub(crate) device: Uuid,
    pub(crate) origin: crate::config::origin::Origin,
    pub(crate) tokens: TokenStore,
}

/// Far more than a slow push can build up.
const CHANNEL_ROOM: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(
        "this daemon is not enrolled: run `anima-daemon enrol -` (the code on standard input) first"
    )]
    NotEnrolled,
    #[error("{0}")]
    Home(#[from] HomeError),
    #[error("{0}")]
    Token(#[from] TokenStoreError),
    #[error("{0}")]
    Client(#[from] HubError),
    #[error("sampling stopped")]
    SamplingStopped,
    #[error("the token was issued by another hub than the configuration names")]
    OriginMismatch,
    #[cfg(unix)]
    #[error("{0}")]
    Move(#[from] old_origin::MoveError),
    #[error("the socket of anima-link passes its check now; starting again to move to it")]
    LinkAppeared,
    /// The token is deleted so that `anima-local-hub` enrols the daemon again at its next start (HMD-046, HM-T35).
    #[error("the hub refused the token; it is deleted, and the next start of the hub enrols again")]
    TokenRefused,
    /// The delete of the token waits for the person in the Keychain (R-322), so the token is still stored.
    #[error(
        "the hub refused the token; it is not deleted yet: this computer's daemon needs you to allow it in the Keychain"
    )]
    TokenRefusedKept,
}

#[derive(Debug, PartialEq, Eq)]
enum PushEnd {
    SamplingStopped,
    TokenRefused,
}

/// With `hosted_hub` it reads that container, not the machine (HMD-071), and a refused token ends the run (HM-T35).
///
/// # Errors
/// [`RunError`] when setup fails, sampling ends before `stop`, or with `hosted_hub` the hub refuses the token.
pub async fn run(
    home: &Home,
    hosted_hub: Option<PathBuf>,
    stop: impl Future<Output = ()>,
) -> Result<(), RunError> {
    let mut stop = std::pin::pin!(stop);
    // A token read that waits for the person in the Keychain does not end the run (R-322).
    let credential = tokio::select! {
        read = token_wait::credential(home) => read?,
        () = &mut stop => {
            report::stopping();
            return Ok(());
        }
    };
    let (pusher, device, link) = prepare(home, credential).await?;
    let (sender, receiver) = mpsc::channel(CHANNEL_ROOM);
    let refusal_ends = hosted_hub.is_some();
    let probing = tokio::spawn(sampling::probe(Platform::current(), sender.clone()));
    let stopping = Arc::new(AtomicBool::new(false));
    let usage = tokio::spawn(count(
        counting(home, |name| std::env::var(name).ok()),
        sender.clone(),
        Arc::clone(&stopping),
    ));
    let source = sampling::source(hosted_hub);
    #[cfg(unix)]
    let publishing = tokio::spawn(thread_sync::publish(
        home.clone(),
        sender.clone(),
        pusher.runtime_target(),
    ));
    #[cfg(target_os = "linux")]
    let mut managed = managed_tasks(home, pusher.runtime_target());
    let mut sampling = tokio::spawn(sampling::sample(source, sender));
    report::running(device);
    let outcome = tokio::select! {
        ended = push(pusher, receiver, refusal_ends) => match ended {
            PushEnd::SamplingStopped => Err(RunError::SamplingStopped),
            PushEnd::TokenRefused => Err(forget_refused(home)),
        },
        () = link_wait::passed(&link) => Err(RunError::LinkAppeared),
        ended = &mut sampling => {
            report::sampling_ended(&ended);
            Err(RunError::SamplingStopped)
        }
        () = &mut stop => {
            report::stopping();
            Ok(())
        }
    };
    sampling.abort();
    // Aborting drops a probe that is running, and its child with it (`kill_on_drop`).
    probing.abort();
    // A read on a blocking thread ignores an abort; the flag ends it between two lines.
    stopping.store(true, Ordering::Relaxed);
    usage.abort();
    #[cfg(unix)]
    publishing.abort();
    #[cfg(target_os = "linux")]
    managed.abort_all();
    outcome
}

#[cfg(target_os = "linux")]
fn managed_tasks(home: &Home, target: HubTarget) -> tokio::task::JoinSet<()> {
    let mut tasks = tokio::task::JoinSet::new();
    tasks.spawn(managed_code::run(home.clone(), target.clone()));
    tasks.spawn(workspaces::run(home.clone(), target.clone()));
    tasks.spawn(workspaces::handoffs::run(target.clone()));
    tasks.spawn(source_work::run(home.clone(), target.clone()));
    tasks.spawn(managed_transcripts::run(home.clone(), target));
    tasks
}

#[cfg_attr(
    windows,
    expect(
        clippy::unused_async,
        reason = "only the move of an old enrolment awaits, and Windows has none"
    )
)]
async fn prepare(
    home: &Home,
    credential: Option<Credential>,
) -> Result<(Pusher, Uuid, LinkWatch), RunError> {
    let config = home.read_config()?.ok_or(RunError::NotEnrolled)?;
    let store = TokenStore::open(home);
    let credential = credential.ok_or(RunError::NotEnrolled)?;
    #[cfg(unix)]
    let old_origin::Start {
        enrolled: old_origin::Enrolled { config, credential },
        watch: link,
        socket,
    } = old_origin::at_start(home, &store, old_origin::Enrolled { config, credential }).await?;
    #[cfg(windows)]
    let (link, socket) = ((), home.link_socket());
    if credential.hub != config.hub {
        return Err(RunError::OriginMismatch);
    }
    let hub = HubClient::reaching(config.hub.reach(), &socket, TIMEOUT)?;
    Ok((
        Pusher::new(hub, config.device, credential, store)
            .with_keys(Keeper::open(KeyStore::open(home)))
            .with_marks(home.clone()),
        config.device,
        link,
    ))
}

/// The key cache went with the refusal (R-147); a delete that waits in the Keychain leaves the token stored.
fn forget_refused(home: &Home) -> RunError {
    refusal_end(TokenStore::open(home).forget(), || {
        if let Err(error) = crate::keychain_wait::mark(home, true) {
            report::keychain_mark_refused(&error);
        }
    })
}

fn refusal_end(forgot: Result<(), TokenStoreError>, mark: impl FnOnce()) -> RunError {
    match forgot {
        Ok(()) => RunError::TokenRefused,
        Err(TokenStoreError::KeychainWaits) => {
            report::forget_waits();
            mark();
            RunError::TokenRefusedKept
        }
        Err(error) => error.into(),
    }
}

async fn push(
    mut pusher: Pusher,
    mut events: mpsc::Receiver<Event>,
    refusal_ends: bool,
) -> PushEnd {
    let mut next = Instant::now() + RHYTHM;
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(event) => pusher.accept(event),
                None => return PushEnd::SamplingStopped,
            },
            () = sleep_until(next) => {
                let wait = pusher.push_due(Utc::now()).await;
                if refusal_ends && pusher.is_stopped() {
                    return PushEnd::TokenRefused;
                }
                while let Ok(event) = events.try_recv() {
                    pusher.accept(event);
                }
                next = Instant::now() + wait;
            }
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests;

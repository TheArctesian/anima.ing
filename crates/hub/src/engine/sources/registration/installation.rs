//! Finding the installation of the App the attempt made, and the task that watches the attempt (K-E1 Section 10a).
//! An installation ID from a browser redirect is not trusted: the hub takes it only when GitHub, asked with the App's own token, says it `fits`.

use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use ring::rand::SystemRandom;
use source_exchange::{Declaration, Installation};
use tokio::task::JoinHandle;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{
    GITHUB,
    attempt::{App, Finished, Phase},
    attempts::Attempts,
    finish::finish,
};
use crate::{
    clock::Clock,
    engine::sources::{
        Scheduling,
        acquisition::{
            app_key::AppSigner,
            installations::{Lookup, installation, installations},
        },
    },
};

#[must_use]
pub fn fits(found: &Installation, app: &App) -> bool {
    found.app_id == app.id && found.account_id == app.owner.id && !found.suspended
}

fn signer(app: &App, key: &Zeroizing<String>) -> Option<AppSigner> {
    AppSigner::read(app.id, key.as_bytes()).ok()
}

async fn verified(
    scheduling: &Scheduling,
    (app, key): (&App, &Zeroizing<String>),
    id: &str,
) -> Option<u64> {
    let declaration: Declaration = *scheduling.registry.declaration(GITHUB)?;
    let signer = signer(app, key)?;
    let now = scheduling.clock.now();
    let random = SystemRandom::new();
    let asked = installation(
        &scheduling.transport,
        &declaration,
        (&signer, id),
        (now, &random),
    )
    .await;
    match asked {
        Lookup::Found(found) if fits(&found, app) && found.id.to_string() == id => Some(found.id),
        _ => None,
    }
}

/// When this request does not end the attempt, it wakes the watch and waits up to [`Attempts::ended_within`], so the page shows how the attempt ended.
pub async fn after_install(
    scheduling: &Scheduling,
    attempts: &Attempts,
    id: Uuid,
    (waiting, installation): (Option<(App, Zeroizing<String>)>, Option<&str>),
) -> Option<Phase> {
    let found = match (waiting, installation) {
        (Some((app, key)), Some(installation)) => {
            verified(scheduling, (&app, &key), installation).await
        }
        _ => None,
    };
    let stored = match found {
        Some(found) => finish(scheduling, attempts, (id, found)).await,
        None => None,
    };
    match stored {
        Some(Finished::Connected { .. }) => Some(Phase::Connected),
        Some(Finished::Failed(_)) => Some(Phase::Failed),
        Some(Finished::Again) | None => {
            attempts.wake();
            attempts.ended_within(id, scheduling.clock.as_ref()).await
        }
    }
}

async fn listed(scheduling: &Scheduling, (app, key): (&App, &Zeroizing<String>)) -> Lookup<u64> {
    let (Some(declaration), Some(signer)) = (
        scheduling.registry.declaration(GITHUB).copied(),
        signer(app, key),
    ) else {
        return Lookup::Unusable;
    };
    let now = scheduling.clock.now();
    let random = SystemRandom::new();
    match installations(&scheduling.transport, &declaration, &signer, (now, &random)).await {
        Lookup::Found(all) => (all.iter().find(|found| fits(found, app)))
            .map_or(Lookup::NotFound, |found| Lookup::Found(found.id)),
        Lookup::NotFound => Lookup::NotFound,
        Lookup::Unreachable => Lookup::Unreachable,
        Lookup::Unusable => Lookup::Unusable,
    }
}

pub const BACKOFF_CAP: Duration = Duration::from_secs(300);

/// The poll interval doubles for each unanswered ask, up to [`BACKOFF_CAP`]; a wake never asks sooner than one wait after the last ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pace {
    poll: Duration,
    last: Option<DateTime<Utc>>,
    misses: u32,
}

impl Pace {
    #[must_use]
    pub const fn new(poll: Duration) -> Self {
        Self {
            poll,
            last: None,
            misses: 0,
        }
    }

    #[must_use]
    pub fn wait(&self) -> Duration {
        let doubled = (1_u32.checked_shl(self.misses))
            .map_or(Duration::MAX, |factor| self.poll.saturating_mul(factor));
        doubled.min(BACKOFF_CAP.max(self.poll))
    }

    #[must_use]
    pub fn woken_may_ask(&self, now: DateTime<Utc>) -> bool {
        let wait = TimeDelta::from_std(self.wait()).unwrap_or(TimeDelta::MAX);
        self.last.is_none_or(|last| {
            last.checked_add_signed(wait)
                .is_some_and(|next| now >= next)
        })
    }

    pub const fn asked(&mut self, now: DateTime<Utc>, answered: bool) {
        self.last = Some(now);
        self.misses = if answered {
            0
        } else {
            self.misses.saturating_add(1)
        };
    }
}

async fn next_ask(attempts: &Attempts, pace: &Pace, clock: &dyn Clock) {
    let due = tokio::time::Instant::now() + pace.wait();
    loop {
        tokio::select! {
            () = tokio::time::sleep_until(due) => return,
            () = attempts.woken() => {
                if pace.woken_may_ask(clock.now()) {
                    return;
                }
            }
        }
    }
}

pub fn watch(scheduling: Scheduling, attempts: Attempts, id: Uuid) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut pace = Pace::new(attempts.poll());
        loop {
            next_ask(&attempts, &pace, scheduling.clock.as_ref()).await;
            let now = scheduling.clock.now();
            let seen = attempts.step(Some(id), now, |held| Ok((held.ended(), held.waiting_app())));
            let Ok((false, waiting)) = seen else {
                return;
            };
            let Some((app, key)) = waiting else {
                continue;
            };
            let listed = listed(&scheduling, (&app, &key)).await;
            pace.asked(now, listed != Lookup::Unreachable);
            if let Lookup::Found(found) = listed {
                finish(&scheduling, &attempts, (id, found)).await;
            }
        }
    })
}

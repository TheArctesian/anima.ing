//! The last exposition built (D-24): scrapes during a build wait for that build (single flight).
//! The build runs in its own task, so a scrape the client abandons neither cancels it nor starts another.

use std::{
    future::Future,
    sync::{Mutex, PoisonError},
    time::Duration,
};

use axum::body::Bytes;
use chrono::{DateTime, TimeDelta, Utc};
use tokio::sync::watch;

/// How long a built exposition is served before a scrape builds the next (D-24).
pub(super) const FRESH_FOR: TimeDelta = TimeDelta::seconds(10);
/// How long a failed build answers scrapes before a scrape builds again (D-24).
pub(super) const FAILED_FOR: TimeDelta = TimeDelta::seconds(2);
/// How long a build may run before its round fails, so a hung statement ends.
pub(super) const BUILD_LIMIT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
enum Outcome {
    Built(Bytes),
    Failed,
}

type Ending = watch::Receiver<Option<Outcome>>;

#[derive(Debug)]
struct Round {
    at: DateTime<Utc>,
    ending: Ending,
}

impl Round {
    /// A task that ended without an outcome (it panicked) spends the round, and so does a clock
    /// moved back by the time kept.
    fn spent(&self, now: DateTime<Utc>) -> bool {
        let outcome = self.ending.borrow().clone();
        let kept_for = match outcome {
            None => return self.ending.has_changed().is_err(),
            Some(Outcome::Built(_)) => FRESH_FOR,
            Some(Outcome::Failed) => FAILED_FOR,
        };
        (now - self.at).abs() >= kept_for
    }
}

#[derive(Debug, Default)]
pub(super) struct Cache {
    current: Mutex<Option<Round>>,
}

impl Cache {
    /// Dropping the returned future leaves the build running.
    pub(super) async fn body<F, Fut>(&self, now: DateTime<Utc>, build: F) -> Option<Bytes>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<String>> + Send + 'static,
    {
        let mut ending = self.join(now, build);
        let outcome = ending.wait_for(Option::is_some).await.ok()?.clone();
        match outcome {
            Some(Outcome::Built(body)) => Some(body),
            Some(Outcome::Failed) | None => None,
        }
    }

    fn join<F, Fut>(&self, now: DateTime<Utc>, build: F) -> Ending
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<String>> + Send + 'static,
    {
        let mut current = self.current.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(round) = current.as_ref().filter(|round| !round.spent(now)) {
            return round.ending.clone();
        }
        let (sender, ending) = watch::channel(None);
        *current = Some(Round {
            at: now,
            ending: ending.clone(),
        });
        drop(current);
        tokio::spawn(run(build(), sender));
        ending
    }
}

async fn run(build: impl Future<Output = Option<String>>, sender: watch::Sender<Option<Outcome>>) {
    let outcome = match tokio::time::timeout(BUILD_LIMIT, build).await {
        Ok(Some(text)) => Outcome::Built(Bytes::from(text)),
        Ok(None) => Outcome::Failed,
        Err(elapsed) => {
            // A task's root is its boundary: no caller is left to hear of the timeout.
            tracing::error!(%elapsed, "the metrics exposition build took too long");
            Outcome::Failed
        }
    };
    sender.send_replace(Some(outcome));
}

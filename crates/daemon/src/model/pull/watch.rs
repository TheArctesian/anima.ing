//! The reports of a running model item (K11 Section 3): each 15 seconds, and for a pull with its
//! progress (PULL-007). They hold its lease, and their answer carries a stop (K10 Section 7).

use std::time::Duration;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::order::{Order, Parameters};
use daemon_core::model_work::pull::{progress, total_bytes};
use daemon_core::wire::{DaemonToken, RefusalCode};
use serde_json::{Value, json};
use tokio::time::Instant;

use crate::hub::HubClient;
use crate::hub::answer::Answer;
use crate::model::cycle::Node;

pub const EVERY: Duration = Duration::from_secs(15);

/// With no report that the hub accepted for this time, the hub gave the item up: the pull ends
/// (K10 Section 7, SF-005).
pub const LEASE: Duration = Duration::from_secs(90);

/// How the step of an item ended.
#[derive(Debug, PartialEq)]
pub enum Watched {
    Ended(Result<Option<Value>, Cause>),
    /// The hub told a stop: the item is reported `stopped`.
    Stopped,
    /// The machine no longer holds the item: nothing more is reported.
    Lost,
}

/// The refusals of a report that end the item at once (K10 Section 7).
const fn ends(code: RefusalCode) -> bool {
    matches!(
        code,
        RefusalCode::NotHolder
            | RefusalCode::TokenRefused
            | RefusalCode::WrongDevice
            | RefusalCode::WorkOff
    )
}

/// Runs the step of `order` and reports it at each `node.report_every`, so that a long step (the
/// hash of a large found model, a pull) keeps its lease; a `model_pull` adds its progress. A stop
/// or the loss of the item drops the step, which ends a pull process; its partial files stay for
/// the next pull of the copy.
pub async fn run(
    node: &mut Node,
    order: &Order,
    client: &HubClient,
    token: &DaemonToken,
) -> Watched {
    let pulled = match &order.parameters {
        Parameters::Pull { copy, files, .. } => Some((*copy, total_bytes(files))),
        _ => None,
    };
    let models = node.folders.models.clone();
    let measure = move || pulled.map_or(0, |(copy, _)| super::bytes_of(&models, copy));
    let every = node.report_every;
    let work = node.run(order);
    tokio::pin!(work);
    let mut ticks = tokio::time::interval_at(Instant::now() + every, every);
    let (mut before, mut accepted) = ((Instant::now(), measure()), Instant::now());
    loop {
        tokio::select! {
            outcome = &mut work => return Watched::Ended(outcome),
            _ = ticks.tick() => {
                let now = (Instant::now(), measure());
                let seconds = now.0.duration_since(before.0).as_secs();
                let mut body = json!({ "id": order.id, "attempt": order.attempt, "state": "running" });
                if let Some((_, total)) = pulled {
                    body["progress"] = json!(progress(now.1, before.1, total, seconds));
                }
                before = now;
                match client.report_work(token, &body).await {
                    Answer::Accepted(reported) if reported.stop.is_some() => return Watched::Stopped,
                    Answer::Accepted(_) => accepted = Instant::now(),
                    Answer::Refused(code) if ends(code) => return Watched::Lost,
                    _ if accepted.elapsed() >= LEASE => return Watched::Lost,
                    _ => {}
                }
            }
        }
    }
}

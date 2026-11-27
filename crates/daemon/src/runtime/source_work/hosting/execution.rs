//! Dispatch a prepared action once and renew its lease while the command runs.
use std::{
    pin::pin,
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::Utc;
use daemon_core::wire::{DaemonToken, Held, RefusalCode, StopReason, WorkState};
use serde_json::Value;

use crate::{
    applications::{self, Client, Offer, Outcome, Prepared},
    hub::{HubClient, answer::Answer, work::ApplicationStartError},
};

use super::{State, apply, reports, write};

pub(super) struct Authority {
    pub hub: Arc<HubClient>,
    pub token: Arc<DaemonToken>,
}

pub(super) async fn run(
    state: State,
    client: Arc<Client>,
    authority: Authority,
    (held, item): (Held, Value),
) {
    if item["type"] == "application_apply" {
        return apply::run(state, &client, &authority, (held, &item)).await;
    }
    let prepared = match client.prepare(&item).await {
        Ok(Offer::Ready(prepared)) => prepared,
        Ok(Offer::Refused(cause) | Offer::Failed(cause)) => {
            return record_refusal(&state, held, &cause);
        }
        Err(error) => {
            let cause = if matches!(error, applications::Error::Protocol) {
                "member_unknown"
            } else {
                "applications_off"
            };
            return record_refusal(&state, held, cause);
        }
    };
    let preparing = reports::phase(held, WorkState::Preparing);
    let Answer::Accepted(ack) = reports::send(&authority.hub, &authority.token, &preparing).await
    else {
        write(&state, |journal| journal.settle(held, Utc::now()));
        return;
    };
    if let Some(stop) = ack.stop {
        write(&state, |journal| {
            journal.result(reports::stopped(held, stop.reason, 0))
        });
        return;
    }
    if !write(&state, |journal| journal.fence(held)) {
        return;
    }
    settle_execution(&state, authority, prepared).await;
}

fn record_refusal(state: &State, held: Held, cause: &str) {
    write(state, |journal| {
        journal.result(reports::terminal(held, WorkState::Refused, Some(cause), 0))
    });
}

async fn settle_execution(state: &State, authority: Authority, prepared: Prepared) {
    let started = Instant::now();
    let held = prepared.item();
    let (result, interruption) = run_with_lease(&authority, prepared).await;
    let seconds = started.elapsed().as_secs();
    let stop = match interruption {
        Some(Interruption::Lost) => {
            write(state, |journal| journal.settle(held, Utc::now()));
            return;
        }
        Some(Interruption::Stop(reason)) => Some(reason),
        None => None,
    };
    let report = stop.map_or_else(
        || match result {
            Ok(Outcome::Succeeded) => reports::terminal(held, WorkState::Done, None, seconds),
            Ok(Outcome::Failed(cause)) => {
                reports::terminal(held, WorkState::Failed, Some(&cause), seconds)
            }
            Err(ApplicationStartError::Stopped(reason)) => reports::stopped(held, reason, seconds),
            Err(ApplicationStartError::Local(applications::Error::NotAuthorized)) => {
                reports::stopped(held, StopReason::Expired, seconds)
            }
            Err(_) => reports::terminal(held, WorkState::Failed, Some("orphan"), seconds),
        },
        |reason| reports::stopped(held, reason, seconds),
    );
    write(state, |journal| journal.result(report));
}

enum Interruption {
    Stop(StopReason),
    Lost,
}

async fn run_with_lease(
    authority: &Authority,
    prepared: Prepared,
) -> (Result<Outcome, ApplicationStartError>, Option<Interruption>) {
    // The initial Running POST in start_application is bounded by the hub client's timeout.
    let mut heartbeats = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(15),
        Duration::from_secs(10),
    );
    heartbeats.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let running = reports::phase(prepared.item(), WorkState::Running);
    let mut execution = pin!(authority.hub.start_application(&authority.token, prepared));
    let mut interruption = None;
    loop {
        tokio::select! {
            biased;
            result = &mut execution => return (result, interruption),
            _ = heartbeats.tick(), if interruption.is_none() => {
                interruption = match reports::send(&authority.hub, &authority.token, &running).await {
                    Answer::Accepted(ack) => ack.stop.map(|stop| Interruption::Stop(stop.reason)),
                    Answer::Refused(code) if relinquished(code) => Some(Interruption::Lost),
                    _ => None,
                };
            }
        }
    }
}

pub(super) const fn relinquished(code: RefusalCode) -> bool {
    matches!(
        code,
        RefusalCode::NotHolder
            | RefusalCode::WorkOff
            | RefusalCode::TokenRefused
            | RefusalCode::WrongDevice
    )
}

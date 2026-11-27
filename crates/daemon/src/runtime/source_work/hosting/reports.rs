//! The wire states of a host action, with no isolated job resources.
use anima_applications_wire::Health;
use daemon_core::wire::{
    ApplicationHealth, Cleanup, DaemonToken, Held, Reported, StopReason, WorkReport, WorkResult,
    WorkState,
};

use crate::hub::{
    HubClient,
    answer::{Answer, Cause},
};

pub(super) const fn phase(item: Held, state: WorkState) -> WorkReport {
    WorkReport {
        id: item.id,
        attempt: item.attempt,
        state,
        log: None,
        result: None,
    }
}

pub(super) fn terminal(
    item: Held,
    state: WorkState,
    cause: Option<&str>,
    seconds: u64,
) -> WorkReport {
    let outcome = if state == WorkState::Done {
        "succeeded"
    } else {
        state.as_str()
    };
    WorkReport {
        result: Some(WorkResult {
            outcome: outcome.to_owned(),
            cause: cause.map(str::to_owned),
            seconds,
            health: None,
            cleanup: Cleanup {
                isolation: "none".to_owned(),
                checkout: "none".to_owned(),
                secrets: "none".to_owned(),
                keychain: "none".to_owned(),
            },
        }),
        ..phase(item, state)
    }
}

pub(super) fn applied(item: Held, health: &Health, seconds: u64) -> WorkReport {
    let mut report = terminal(item, WorkState::Done, None, seconds);
    if let Some(result) = &mut report.result {
        result.health = Some(match health {
            Health::Healthy => ApplicationHealth::Healthy,
            Health::Unhealthy => ApplicationHealth::Unhealthy,
            Health::NoCheck => ApplicationHealth::NoCheck,
            Health::NotWaited => ApplicationHealth::NotWaited,
        });
    }
    report
}

pub(super) fn stopped(item: Held, reason: StopReason, seconds: u64) -> WorkReport {
    let cause = (reason == StopReason::Expired).then_some("limit_seconds");
    terminal(item, WorkState::Stopped, cause, seconds)
}

pub(super) async fn send(
    hub: &HubClient,
    token: &DaemonToken,
    report: &WorkReport,
) -> Answer<Reported> {
    let Ok(body) = serde_json::to_value(report) else {
        return Answer::Failed(Cause::Unreadable);
    };
    hub.report_work(token, &body).await
}

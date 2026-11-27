//! A separate Linux task; remote Code never blocks the sampler or credential renewal.
pub(super) mod broker;
mod outbox;
use super::HubTarget as Target;
use crate::{config::home::Home, hub::answer::Answer};
use agent_threads::managed::{Delivery, LocalRequest, Poll, servers::ServerPoll};
use chrono::Utc;
use outbox::{Entry, Outbox};
use std::time::Duration;
use tokio::time::MissedTickBehavior;

pub(super) async fn run(home: Home, target: Target) {
    if std::env::var("ANIMA_MANAGED_CODE").as_deref() != Ok("1") {
        return;
    }
    let Some(outbox) = Outbox::open(&home) else {
        return;
    };
    let mut ticks = tokio::time::interval(Duration::from_secs(5));
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut cursor = 0_usize;
    loop {
        ticks.tick().await;
        let tokens = target.tokens.clone();
        let Ok(Ok(Some(credential))) = tokio::task::spawn_blocking(move || tokens.load()).await
        else {
            continue;
        };
        if credential.hub != target.origin || credential.expires_at <= Utc::now() {
            continue;
        }
        let Some(mut pending) = pending(&outbox, &target, &mut cursor).await else {
            continue;
        };
        let Some(poll) = make_poll(&outbox, &target, pending.as_ref()).await else {
            continue;
        };
        let Answer::Accepted(delivery) =
            target.hub.poll_managed_code(&credential.token, &poll).await
        else {
            continue;
        };
        if let Some(entry) = pending.take()
            && delivery.acknowledged.contains(&entry.report.request)
            && !outbox.acknowledge(&entry)
        {
            continue;
        }
        deliver(&outbox, &target, delivery).await;
    }
}

async fn make_poll(
    outbox: &Outbox,
    target: &Target,
    pending: Option<&Entry>,
) -> Option<ServerPoll> {
    let mut inventory = broker::inventory().await?;
    if !outbox.room() {
        for offer in &mut inventory.workspaces {
            offer.ready = false;
        }
        inventory.servers.clear();
    }
    let poll = ServerPoll {
        poll: Poll {
            device: target.device,
            workspaces: inventory.workspaces,
            reports: pending.iter().map(|entry| entry.report.clone()).collect(),
        },
        servers: inventory.servers,
    };
    poll.fits().then_some(poll)
}

async fn pending(outbox: &Outbox, target: &Target, cursor: &mut usize) -> Option<Option<Entry>> {
    let ids = outbox.ids()?;
    if ids.is_empty() {
        return Some(None);
    }
    *cursor %= ids.len();
    let mut entry = outbox.read(ids[*cursor])?;
    *cursor += 1;
    // Never send an old hub's content to a new enrolment, even at the same URL.
    if entry.origin != target.origin.as_text() || entry.device != target.device {
        return Some(None);
    }
    if !entry.finished
        && let Some(value) = broker::call(&LocalRequest::Status {
            workspace: entry.report.workspace,
            request: entry.report.request,
            principal: entry.report.principal,
        })
        .await
        && value["state"] == "completed"
    {
        entry.report.result = Some(serde_json::from_value(value["result"].clone()).ok()?);
        entry.finished = true;
        if !outbox.save(&entry) {
            return None;
        }
    }
    Some(Some(entry))
}

async fn deliver(outbox: &Outbox, target: &Target, delivery: Delivery) {
    let (Some(command), Some(valid_until)) = (delivery.command, delivery.valid_until) else {
        return;
    };
    let remaining = valid_until.signed_duration_since(Utc::now());
    if !command.fits()
        || remaining <= chrono::TimeDelta::zero()
        || remaining > chrono::TimeDelta::seconds(15)
    {
        return;
    }
    let Some(mut entry) = outbox.reserve(&command, &target.origin.as_text(), target.device) else {
        return;
    };
    if entry.finished {
        return;
    }
    let expected = command.clone();
    let Some(value) = broker::call(&LocalRequest::Deliver {
        command: Box::new(command),
        valid_until,
    })
    .await
    else {
        return;
    };
    if value["state"] == "completed" {
        let Ok(result) = serde_json::from_value(value["result"].clone()) else {
            return;
        };
        if !expected.accepts(&result) {
            return;
        }
        entry.report.result = Some(result);
        entry.finished = true;
        let _ = outbox.save(&entry);
    }
}

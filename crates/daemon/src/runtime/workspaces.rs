//! Independent workspace lifecycle task; never blocks telemetry or credential renewal.
mod checkout;
pub(super) mod handoffs;
mod outbox;
use super::{HubTarget, owner_socket};
use crate::{config::home::Home, hub::answer::Answer};
use agent_threads::managed::workspace::{
    Action, Capacity, Delivery, LocalRequest, Poll, Receipt, SOCKET,
};
use chrono::Utc;
use daemon_core::wire::DaemonToken;
use outbox::{Entry, Outbox};
use std::time::Duration;

async fn broker(request: &LocalRequest) -> Option<serde_json::Value> {
    owner_socket::call(SOCKET, request, (4096, 8192)).await
}

pub(super) async fn run(home: Home, target: HubTarget) {
    if std::env::var("ANIMA_MANAGED_CODE").as_deref() != Ok("1") {
        return;
    }
    let Some(outbox) = Outbox::open(&home) else {
        return;
    };
    let mut ticks = tokio::time::interval(Duration::from_secs(5));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut cursor = 0;
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
        let Some(pending) = pending(&outbox, &target, &mut cursor).await else {
            continue;
        };
        let (ready, capacity, private_checkout) = readiness(&outbox).await;
        let poll = Poll {
            device: target.device,
            ready,
            report: pending.as_ref().map(|entry| entry.receipt.clone()),
        };
        if !poll.fits() {
            continue;
        }
        let Answer::Accepted(delivery) = target
            .hub
            .poll_workspaces(&credential.token, &poll, (capacity, private_checkout))
            .await
        else {
            continue;
        };
        if let Some(entry) = pending
            && delivery
                .acknowledged
                .contains(&entry.receipt.command.request)
            && !outbox.acknowledge(&entry)
        {
            continue;
        }
        deliver(&outbox, &target, &credential.token, delivery).await;
    }
}

async fn readiness(outbox: &Outbox) -> (bool, Option<Capacity>, bool) {
    if !outbox.room() {
        return (false, None, false);
    }
    let Some(value) = broker(&LocalRequest::Ready).await else {
        return (false, None, false);
    };
    let capacity = value.get("capacity").map(|value| {
        serde_json::from_value::<Capacity>(value.clone())
            .ok()
            .filter(|capacity| capacity.fits())
            .unwrap_or_default()
    });
    (
        value["available"] == true,
        capacity,
        value["privateCheckout"] == true,
    )
}

async fn pending(outbox: &Outbox, target: &HubTarget, cursor: &mut usize) -> Option<Option<Entry>> {
    let ids = outbox.ids()?;
    if ids.is_empty() {
        return Some(None);
    }
    *cursor %= ids.len();
    let mut entry = outbox.read(ids[*cursor])?;
    *cursor += 1;
    if entry.origin != target.origin.as_text() || entry.device != target.device {
        return Some(None);
    }
    if !entry.finished {
        let command = &entry.receipt.command;
        if let Some(value) = broker(&LocalRequest::Status {
            workspace: command.workspace,
            request: command.request,
            principal: command.principal,
        })
        .await
        {
            let receipt: Receipt = serde_json::from_value(value).ok()?;
            if receipt.command != *command || !receipt.fits() {
                return None;
            }
            entry.finished = receipt.result.is_some() || receipt.failure.is_some();
            entry.receipt = receipt;
            if !outbox.save(&entry) {
                return None;
            }
        }
    }
    Some(Some(entry))
}

async fn deliver(outbox: &Outbox, target: &HubTarget, token: &DaemonToken, delivery: Delivery) {
    let (Some(command), Some(valid_until)) = (delivery.command, delivery.valid_until) else {
        return;
    };
    let remaining = valid_until.signed_duration_since(Utc::now());
    if !command.fits()
        || remaining <= chrono::TimeDelta::zero()
        || remaining > chrono::TimeDelta::seconds(15)
        || delivery.checkout_repository.is_some_and(|repository| {
            repository.is_nil() || !matches!(command.action, Action::Start { .. })
        })
    {
        return;
    }
    let Some(mut entry) = outbox.reserve(command.clone(), target.origin.as_text(), target.device)
    else {
        return;
    };
    let value = match delivery.checkout_repository {
        Some(repository_id) => {
            match checkout::deliver((target, token), command, valid_until, repository_id).await {
                checkout::Outcome::Delivered(value) => value,
                checkout::Outcome::Refused => {
                    entry.finished = true;
                    entry.receipt.failure = Some(
                        "The private checkout credential was unavailable. Submit a new start."
                            .to_owned(),
                    );
                    let _ = outbox.save(&entry);
                    return;
                }
            }
        }
        None => {
            broker(&LocalRequest::Deliver {
                command,
                valid_until,
            })
            .await
        }
    };
    let Some(value) = value else { return };
    let Ok(receipt) = serde_json::from_value::<Receipt>(value) else {
        return;
    };
    if receipt.command != entry.receipt.command || !receipt.fits() {
        return;
    }
    entry.finished = receipt.result.is_some() || receipt.failure.is_some();
    entry.receipt = receipt;
    let _ = outbox.save(&entry);
}

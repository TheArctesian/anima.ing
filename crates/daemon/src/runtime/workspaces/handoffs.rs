//! A workspace copy has its own task so lifecycle and telemetry keep their turns.
mod broker;
mod settlement;

use super::super::HubTarget;
use agent_threads::managed::workspace::handoff::{
    Import, Offer, Receipt, Source,
    socket::{self, Command},
};
use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::wire::DaemonToken;
use futures_util::TryStreamExt as _;
use serde_json::Value;
use std::time::Duration;
use tokio::io::AsyncWriteExt as _;

pub(in crate::runtime) async fn run(target: HubTarget) {
    if std::env::var("ANIMA_MANAGED_CODE").as_deref() != Ok("1") {
        return;
    }
    let mut ticks = tokio::time::interval(Duration::from_secs(5));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
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
        let Some(offer) = target.hub.poll_handoffs(&credential.token).await else {
            continue;
        };
        deliver(&target, &credential.token, offer).await;
    }
}

async fn deliver(target: &HubTarget, token: &DaemonToken, offer: Offer) {
    match offer {
        Offer::Export {
            source,
            expires_at,
            staged,
        } => {
            let _ = within(
                expires_at,
                export(target, token, (source, expires_at), staged),
            )
            .await;
        }
        Offer::Restore { import, expires_at } if import.destination_device == target.device => {
            let _ = within(expires_at, restore(target, token, import, expires_at)).await;
        }
        Offer::Activate { import, expires_at } if import.destination_device == target.device => {
            let _ = within(
                expires_at,
                activate(target, token, import, (expires_at, false)),
            )
            .await;
        }
        Offer::InspectActivation { import, expires_at }
            if import.destination_device == target.device =>
        {
            let _ = within(
                expires_at,
                activate(target, token, import, (expires_at, true)),
            )
            .await;
        }
        Offer::Discard { import, expires_at } if import.destination_device == target.device => {
            let _ = within(
                expires_at,
                settlement::discard(target, token, import, expires_at),
            )
            .await;
        }
        Offer::Withdraw { source, expires_at } if source.binding.destination != target.device => {
            let _ = within(
                expires_at,
                settlement::withdraw(target, token, source, expires_at),
            )
            .await;
        }
        Offer::Retire { source, expires_at } if source.binding.destination != target.device => {
            let _ = within(
                expires_at,
                settlement::retire(target, token, source, expires_at),
            )
            .await;
        }
        _ => (),
    }
}

async fn within(
    expires_at: DateTime<Utc>,
    action: impl std::future::Future<Output = Option<()>>,
) -> Option<()> {
    let left = expires_at.signed_duration_since(Utc::now());
    if left <= TimeDelta::zero() || left > TimeDelta::hours(1) {
        return None;
    }
    tokio::time::timeout(left.to_std().ok()?, action)
        .await
        .ok()
        .flatten()
}

async fn export(
    target: &HubTarget,
    token: &DaemonToken,
    (source, expires_at): (Source, DateTime<Utc>),
    staged: Option<Receipt>,
) -> Option<()> {
    if !source.fits() || source.binding.destination == target.device {
        return None;
    }
    let replay = staged.is_some();
    let receipt = fence(source, expires_at, staged).await?;
    let (mut bytes, completed) = broker::export(&receipt, expires_at).await?;
    if replay {
        while bytes.try_next().await.ok()?.is_some() {}
    } else if !target.hub.upload_checkpoint(token, &receipt, bytes).await {
        return None;
    }
    completed.await.ok()?;
    target
        .hub
        .confirm_checkpoint(token, &receipt)
        .await
        .then_some(())
}

async fn fence(
    source: Source,
    expires_at: DateTime<Utc>,
    staged: Option<Receipt>,
) -> Option<Receipt> {
    let fenced = broker::control(
        Command::Fence {
            workspace: source.workspace,
            generation: source.generation,
            principal: source.principal,
            binding: source.binding,
        },
        expires_at,
    )
    .await?;
    if fenced["state"] != "fenced"
        || fenced["binding"] != serde_json::to_value(source.binding).ok()?
    {
        return None;
    }
    let receipt = match staged {
        Some(receipt) => receipt,
        None => {
            serde_json::from_value(broker::control(Command::Describe { source }, expires_at).await?)
                .ok()?
        }
    };
    (receipt.source == source
        && receipt.fits(agent_threads::managed::workspace::Capacity::MAXIMUM.storage_bytes))
    .then_some(receipt)
}

async fn restore(
    target: &HubTarget,
    token: &DaemonToken,
    import: Import,
    expires_at: DateTime<Utc>,
) -> Option<()> {
    if !import.fits() {
        return None;
    }
    restore_checkpoint(target, token, &import, expires_at).await?;
    target
        .hub
        .report_checkpoint_restored(token, &import)
        .await
        .then_some(())
}

async fn restore_checkpoint(
    target: &HubTarget,
    token: &DaemonToken,
    import: &Import,
    expires_at: DateTime<Utc>,
) -> Option<()> {
    let receipt = &import.receipt;
    if broker::control(
        Command::RestoreStatus {
            import: import.clone(),
        },
        expires_at,
    )
    .await
    .is_some_and(|value| restored(&value, receipt))
    {
        return Some(());
    }
    let (mut input, mut output) = broker::open(
        Command::Restore {
            import: import.clone(),
        },
        expires_at,
    )
    .await?;
    let phase = broker::reply(&mut input).await?;
    if restored(&phase, receipt) {
        return Some(());
    }
    if phase["stream"] != socket::RESTORE {
        return None;
    }
    download(target, token, receipt, &mut output).await?;
    let phase = broker::reply(&mut input).await?;
    restored(&phase, receipt).then_some(())
}

async fn download(
    target: &HubTarget,
    token: &DaemonToken,
    receipt: &Receipt,
    output: &mut tokio::net::unix::OwnedWriteHalf,
) -> Option<()> {
    let mut checkpoint = target.hub.download_checkpoint(token, receipt).await?;
    let mut remaining = receipt.bytes;
    while let Some(chunk) = checkpoint.bytes.try_next().await.ok()? {
        remaining = remaining.checked_sub(u64::try_from(chunk.len()).ok()?)?;
        output.write_all(&chunk).await.ok()?;
    }
    if remaining != 0 {
        return None;
    }
    output.shutdown().await.ok()
}

async fn activate(
    target: &HubTarget,
    token: &DaemonToken,
    import: Import,
    (expires_at, inspect_only): (DateTime<Utc>, bool),
) -> Option<()> {
    if !import.fits() {
        return None;
    }
    let command = if inspect_only {
        Command::RestoreStatus {
            import: import.clone(),
        }
    } else {
        Command::Activate {
            import: import.clone(),
        }
    };
    let phase = broker::control(command, expires_at).await?;
    if phase["state"] != "activated" || !restored(&phase, &import.receipt) {
        return None;
    }
    target
        .hub
        .report_checkpoint_activated(token, &import)
        .await
        .then_some(())
}

fn restored(phase: &Value, receipt: &Receipt) -> bool {
    matches!(phase["state"].as_str(), Some("restored" | "activated"))
        && phase["workspace"].as_str().and_then(|id| id.parse().ok())
            == Some(receipt.source.workspace)
        && phase["transfer"].as_str().and_then(|id| id.parse().ok())
            == Some(receipt.source.binding.transfer)
        && phase["digest"] == receipt.digest
}

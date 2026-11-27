use super::{HubTarget, broker, restored};
use agent_threads::managed::workspace::handoff::{Import, Source, Withdrawal, socket::Command};
use chrono::{DateTime, Utc};
use daemon_core::wire::DaemonToken;

pub(super) async fn discard(
    target: &HubTarget,
    token: &DaemonToken,
    import: Import,
    expires_at: DateTime<Utc>,
) -> Option<()> {
    let phase = broker::control(
        Command::Discard {
            import: import.clone(),
        },
        expires_at,
    )
    .await?;
    if phase["state"] == "activated" && restored(&phase, &import.receipt) {
        return target
            .hub
            .report_checkpoint_activated(token, &import)
            .await
            .then_some(());
    }
    if phase["state"] != "discarded" || phase["import"] != serde_json::to_value(&import).ok()? {
        return None;
    }
    target
        .hub
        .report_withdrawal(token, &Withdrawal::Discarded { import })
        .await
        .then_some(())
}

pub(super) async fn withdraw(
    target: &HubTarget,
    token: &DaemonToken,
    source: Source,
    expires_at: DateTime<Utc>,
) -> Option<()> {
    let phase = broker::control(Command::Withdraw { source }, expires_at).await?;
    if phase["state"] != "withdrawn" || phase["source"] != serde_json::to_value(source).ok()? {
        return None;
    }
    target
        .hub
        .report_withdrawal(
            token,
            &Withdrawal::Withdrawn {
                source,
                stopped: serde_json::from_value(phase["stopped"].clone()).ok()?,
            },
        )
        .await
        .then_some(())
}

pub(super) async fn retire(
    target: &HubTarget,
    token: &DaemonToken,
    source: Source,
    expires_at: DateTime<Utc>,
) -> Option<()> {
    let phase = broker::control(Command::Retire { source }, expires_at).await?;
    if phase["state"] != "retired" || phase["source"] != serde_json::to_value(source).ok()? {
        return None;
    }
    target
        .hub
        .report_retirement(token, source)
        .await
        .then_some(())
}

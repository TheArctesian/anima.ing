//! Recover enrolled source work, report leases and deliver verified artifacts.
use super::HubTarget;
use crate::{
    config::home::Home,
    hub::answer::Answer,
    source_work::{Coordinator, Transfer},
    token_store::Credential,
};
use chrono::Utc;
use daemon_core::wire::RefusalCode;
use daemon_core::{artifact::Receipt, wire::DaemonToken};
use std::time::Duration;
use tokio::task::JoinSet;
use uuid::Uuid;
mod checkout;
mod claiming;
mod hosting;

pub(super) async fn run(home: Home, target: HubTarget) {
    if std::env::var("ANIMA_MANAGED_CODE").as_deref() != Ok("1") {
        return;
    }
    let Ok(owner) = Coordinator::open(&home, target.device, &target.origin) else {
        tracing::warn!("Source-work journal could not be opened; recovery is unavailable.");
        return;
    };
    let hosting = hosting::Hosting::open(&home, &target);
    recover(owner, target, hosting).await;
}

async fn recover(mut owner: Coordinator, target: HubTarget, mut hosting: Option<hosting::Hosting>) {
    let mut ticks = tokio::time::interval(Duration::from_secs(5));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut cadence = Cadence {
        cursor: 0,
        next_claim: tokio::time::Instant::now(),
    };
    let mut delivery = Delivery::default();
    loop {
        tokio::select! {
            biased;
            completed = delivery.uploads.task.join_next(), if delivery.uploads.active.is_some() => {
                delivery.uploads.active = None;
                if let Some(Ok((transfer, Answer::Accepted(receipt)))) = completed {
                    let _ = owner.retain_receipt(&transfer, receipt);
                }
            }
            _ = ticks.tick() => {
                if cadence.tick((&mut owner, hosting.as_mut()), &target, &mut delivery).await.is_err() {
                    tracing::warn!("Work journal requires reopening; recovery stopped.");
                    return;
                }
            }
        }
    }
}

struct Cadence {
    cursor: usize,
    next_claim: tokio::time::Instant,
}

impl Cadence {
    async fn tick(
        &mut self,
        (owner, hosting): (&mut Coordinator, Option<&mut hosting::Hosting>),
        target: &HubTarget,
        delivery: &mut Delivery,
    ) -> Result<(), crate::source_work::Error> {
        let now = tokio::time::Instant::now();
        if now >= self.next_claim {
            self.next_claim = now + Duration::from_secs(10);
            claiming::poll(owner, target, hosting).await?;
        }
        step(owner, target, &mut self.cursor, delivery).await
    }
}

async fn step(
    owner: &mut Coordinator,
    target: &HubTarget,
    cursor: &mut usize,
    delivery: &mut Delivery,
) -> Result<(), crate::source_work::Error> {
    let executions = owner.executions()?;
    if executions.is_empty() {
        owner.prune(cursor, Utc::now()).await?;
        return Ok(());
    }
    *cursor %= executions.len();
    let execution = executions[*cursor];
    *cursor += 1;
    let next = if owner.reconcile_retirement(execution, Utc::now()).await? {
        None
    } else {
        deliver(owner, target, execution).await
    };
    delivery.update(owner, target, next)
}

#[derive(Default)]
struct Delivery {
    uploads: Uploads,
    checkout: checkout::Pending,
}

struct Observation {
    execution: Uuid,
    status: serde_json::Value,
    token: DaemonToken,
}

impl Delivery {
    fn update(
        &mut self,
        owner: &mut Coordinator,
        target: &HubTarget,
        next: Option<Observation>,
    ) -> Result<(), crate::source_work::Error> {
        let artifact = if let Some(observed) = next {
            if let Some(transfer) = owner.artifact(observed.execution, &observed.status) {
                Some((transfer, observed.token))
            } else {
                self.checkout.update(owner, target, observed)?;
                None
            }
        } else {
            None
        };
        self.uploads.update(owner, target, artifact);
        Ok(())
    }
}

#[derive(Default)]
struct Uploads {
    active: Option<Uuid>,
    task: JoinSet<(Transfer, Answer<Receipt>)>,
}

impl Uploads {
    fn update(
        &mut self,
        owner: &Coordinator,
        target: &HubTarget,
        next: Option<(Transfer, DaemonToken)>,
    ) {
        if self
            .active
            .is_some_and(|execution| !owner.wants_artifact(execution))
        {
            self.task.abort_all();
        }
        if let Some((transfer, token)) = next.filter(|_| self.active.is_none()) {
            self.active = Some(transfer.execution);
            let hub = target.hub.clone();
            self.task.spawn(async move {
                let result = hub
                    .upload_artifact(&token, &transfer.request, &transfer.offer)
                    .await;
                (transfer, result)
            });
        }
    }
}

#[cfg(test)]
mod tests;

async fn deliver(
    owner: &mut Coordinator,
    target: &HubTarget,
    execution: Uuid,
) -> Option<Observation> {
    let credential = credential(target).await?;
    let Ok(report) = owner.prepare_report(execution).await else {
        return None;
    };
    let Ok(body) = serde_json::to_value(&report) else {
        return None;
    };
    match target.hub.report_work(&credential.token, &body).await {
        Answer::Accepted(response) => {
            if owner.acknowledge(execution, &report, response).is_ok() {
                let status = owner.reconcile(execution).await.ok()?;
                if status["phase"] == "registered" && response.stop.is_none() {
                    let _ = owner.start(execution).await;
                }
                return Some(Observation {
                    execution,
                    status,
                    token: credential.token,
                });
            }
        }
        Answer::Refused(RefusalCode::NotHolder) => {
            let _ = owner.note_lost(execution);
        }
        Answer::Refused(
            RefusalCode::WorkOff | RefusalCode::TokenRefused | RefusalCode::WrongDevice,
        ) => {
            let _ = owner.cancel(execution).await;
        }
        Answer::Refused(_) | Answer::Failed(_) => (),
    }
    None
}

async fn credential(target: &HubTarget) -> Option<Credential> {
    let tokens = target.tokens.clone();
    let credential = tokio::task::spawn_blocking(move || tokens.load())
        .await
        .ok()?
        .ok()??;
    (credential.hub == target.origin && credential.expires_at > Utc::now()).then_some(credential)
}

//! One claimed outbox row (D-16, NOT-001, NOT-002); no database lock is held while a target answers.
//! A row whose check is gone is still sent: it was decided while the check existed (K2).
//! An unreadable channel gets a delivery and a Warning event, since `notificationChannels` hides it.
//! Every write matches the claim's lease end, so a claimer that lost its lease changes only its delivery.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use super::{
    Dispatcher, Fault,
    failure::{Failure, delivery},
    retry::{self, Next},
};
use crate::{
    fleet::{
        events::{self, ActorKind, NewEvent, Severity, Source},
        notifications::{
            Delivery, InstallationSettings, NotificationKind, Outbox, store, template::Values,
        },
    },
    ids::DeliveryId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Finished {
    Sent,
    Ended,
    Failed(Failure),
    OutOfTries,
}

impl Dispatcher {
    /// On error the row stays claimed and comes back when its lease ends.
    pub(super) async fn finish(
        &self,
        row: &Outbox,
        defaults: &InstallationSettings,
        now: DateTime<Utc>,
    ) -> Result<Finished, Fault> {
        let Some(channel) = store::list_channels(&self.db, Some(&[row.channel]))
            .await?
            .pop()
        else {
            store::give_up(&self.db, row, now).await?;
            return Ok(Finished::Ended);
        };
        if let Some(check) = row.check
            && store::channel_turned_off(&self.db, check, channel.id).await?
        {
            store::give_up(&self.db, row, now).await?;
            return Ok(Finished::Ended);
        }
        if retry::out_of_tries(row.attempts) {
            let mut tx = self.db.begin().await.map_err(store_fault)?;
            let warning = given_up(&channel.fields.name, row, None);
            self.give_up(&mut tx, row, (now, warning)).await?;
            tx.commit().await.map_err(store_fault)?;
            return Ok(Finished::OutOfTries);
        }
        let values = Values::from_payload(&row.payload);
        let sent = self
            .sender
            .send(&channel, (row.kind, &values), defaults)
            .await;
        let id = DeliveryId::from(Uuid::now_v7());
        let delivery = delivery(id, &channel, (row.kind, now), sent);
        self.record(row, &delivery, sent).await?;
        Ok(sent.map_or_else(Finished::Failed, |()| Finished::Sent))
    }

    async fn record(
        &self,
        row: &Outbox,
        delivery: &Delivery,
        sent: Result<(), Failure>,
    ) -> Result<(), Fault> {
        let now = delivery.at;
        let mut tx = self.db.begin().await.map_err(store_fault)?;
        store::insert_delivery(&mut tx, delivery).await?;
        match sent.map_err(|failure| (failure, retry::after_failure(failure, row.attempts, now))) {
            Ok(()) => {
                store::mark_sent(&mut *tx, row, now).await?;
            }
            Err((_, Next::RetryAt(at))) => {
                store::reschedule(&mut *tx, row, at).await?;
            }
            Err((failure, Next::GiveUp)) => {
                let warning = given_up(&delivery.channel_name, row, Some(failure));
                self.give_up(&mut tx, row, (now, warning)).await?;
            }
        }
        tx.commit().await.map_err(store_fault)?;
        Ok(())
    }

    /// A row is given up once, with one Warning event.
    async fn give_up(
        &self,
        tx: &mut PgConnection,
        row: &Outbox,
        (now, warning): (DateTime<Utc>, NewEvent),
    ) -> Result<(), Fault> {
        if store::give_up(&mut *tx, row, now).await? {
            events::record(tx, &self.bus, now, warning).await?;
        }
        Ok(())
    }
}

fn store_fault(error: sqlx::Error) -> Fault {
    Fault::Store(error.into())
}

const fn kind_words(kind: NotificationKind) -> &'static str {
    match kind {
        NotificationKind::Down => "Down",
        NotificationKind::Up => "Up",
        NotificationKind::StillDown => "Still down",
        NotificationKind::CertificateExpiry => "certificate expiry",
        NotificationKind::DomainExpiry => "domain expiry",
        NotificationKind::Test => "test",
        NotificationKind::Reminder => "reminder",
    }
}

/// Never a target, URL, topic, header or reply (NOT-002, LOG-007).
pub(super) fn given_up(channel_name: &str, row: &Outbox, last: Option<Failure>) -> NewEvent {
    let values = Values::from_payload(&row.payload);
    let about = match (values.check, values.device) {
        (Some(check), Some(device)) => format!(" for {check} on {device}"),
        _ => String::new(),
    };
    // A row out of tries is given up at a claim that sent nothing, so that claim is no try.
    let (tries, why) = match last {
        Some(failure) => (row.attempts, failure_words(failure)),
        None => (
            row.attempts.saturating_sub(1),
            "the outcome of the last try was not recorded".to_owned(),
        ),
    };
    NewEvent {
        severity: Severity::Warning,
        source: Source::Notifications,
        device: None,
        message: format!(
            "Notification channel {} gave up a {} notification{about} after {tries} {}: {why}.",
            channel_name,
            kind_words(row.kind),
            if tries == 1 { "try" } else { "tries" },
        ),
        actor: ActorKind::Hub,
    }
}

fn failure_words(failure: Failure) -> String {
    match failure.code {
        Some(code) => format!("{} {code}", failure.reason.as_str()),
        None => failure.reason.as_str().to_owned(),
    }
}

mod wire;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub(super) use wire::Action;
pub(in crate::fleet) use wire::{Answer, Offer, StopTarget};

use super::{Error, Launch, Owner, attempt, store};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::fleet) struct ControlUpload {
    pub device: Uuid,
    pub launch: Uuid,
    runner: Uuid,
    offer: Offer,
}

pub(in crate::fleet) async fn upload(
    conn: &mut PgConnection,
    owner: Owner,
    request: ControlUpload,
    at: DateTime<Utc>,
) -> Result<(), Error> {
    if request.device != owner.device {
        return Err(Error::Refused("WRONG_DEVICE"));
    }
    if !request.offer.valid() {
        return Err(Error::malformed());
    }
    let launch = store::launch(conn, owner.account, request.launch).await?;
    if launch.device_id != owner.device || launch.runner_id != Some(request.runner) {
        return Err(Error::Refused("LAUNCH_CONFLICT"));
    }
    let receipt = attempt::reconcile(conn, owner, request.launch, at).await?;
    if receipt.binding != "bound" {
        return Err(Error::Refused("THREAD_NOT_FOUND"));
    }
    let value = serde_json::to_value(&request.offer)?;
    let previous: Option<(i64, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT revision, offer FROM agent_threads.control_offers
         WHERE account_id = $1 AND launch_id = $2",
    )
    .bind(owner.account)
    .bind(request.launch)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some((revision, prior)) = previous {
        if revision > request.offer.revision {
            return Ok(());
        }
        if revision == request.offer.revision && prior.as_ref() != Some(&value) {
            return Err(Error::Refused("CONTROL_REVISION_CONFLICT"));
        }
    }
    sqlx::query(
        "INSERT INTO agent_threads.control_offers (account_id, launch_id, revision, offer, expires_at)
         VALUES ($1,$2,$3,$4,$5) ON CONFLICT (account_id, launch_id) DO UPDATE SET
         revision = EXCLUDED.revision, offer = EXCLUDED.offer, expires_at = EXCLUDED.expires_at",
    )
    .bind(owner.account)
    .bind(request.launch)
    .bind(request.offer.revision)
    .bind(value)
    .bind(at + store::FRESH)
    .execute(conn)
    .await?;
    Ok(())
}

pub(in crate::fleet) async fn control(
    db: &PgPool,
    account: Uuid,
    thread: Uuid,
) -> Result<Option<Offer>, Error> {
    let device = thread_device(db, thread).await?;
    let mut tx = db.begin().await?;
    store::lock_existing(&mut tx, device).await?;
    let at = store::now(&mut tx).await?;
    let owner = Owner { account, device };
    let offer = match bound(&mut tx, owner, thread, at).await? {
        Some(launch) => live(&mut tx, owner, launch.id, at).await?,
        None => None,
    };
    tx.commit().await?;
    Ok(offer)
}

pub(super) async fn thread_device<'e>(
    db: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    thread: Uuid,
) -> Result<Uuid, Error> {
    sqlx::query_scalar("SELECT device_id FROM agent_threads.summaries WHERE id = $1")
        .bind(thread)
        .fetch_optional(db)
        .await?
        .ok_or(Error::Refused("THREAD_NOT_FOUND"))
}

pub(super) async fn bound(
    conn: &mut PgConnection,
    owner: Owner,
    thread: Uuid,
    at: DateTime<Utc>,
) -> Result<Option<Launch>, Error> {
    let row: Option<Launch> = sqlx::query_as(
        "SELECT launches.* FROM agent_threads.launches launches
         JOIN agent_threads.summaries summary ON summary.id = launches.thread_id
         WHERE launches.account_id = $1 AND launches.device_id = $2 AND thread_id = $3
           AND NOT withdrawn AND NOT control_closed AND NOT retired AND evidence = 'created'",
    )
    .bind(owner.account)
    .bind(owner.device)
    .bind(thread)
    .fetch_optional(&mut *conn)
    .await?;
    if row.is_some() && !store::credential_is_live(conn, owner.device, at).await? {
        return Ok(None);
    }
    Ok(row)
}

pub(super) async fn live(
    conn: &mut PgConnection,
    owner: Owner,
    launch: Uuid,
    at: DateTime<Utc>,
) -> Result<Option<Offer>, Error> {
    let value: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT offer FROM agent_threads.control_offers
         WHERE account_id = $1 AND launch_id = $2 AND expires_at > $3 AND offer IS NOT NULL",
    )
    .bind(owner.account)
    .bind(launch)
    .bind(at)
    .fetch_optional(conn)
    .await?;
    value
        .map(serde_json::from_value)
        .transpose()
        .map_err(Into::into)
}

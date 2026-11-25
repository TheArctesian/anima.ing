use chrono::{DateTime, TimeDelta, Utc};
use serde::Serialize;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{Error, Launch, Owner};
use crate::{
    fleet::devices::{self, MetricsSource},
    ids::DeviceId,
};

pub(super) const TEN_MINUTES: TimeDelta = TimeDelta::minutes(10);
pub(super) const FRESH: TimeDelta = TimeDelta::seconds(30);

pub(super) async fn now(conn: &mut PgConnection) -> Result<DateTime<Utc>, sqlx::Error> {
    sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(conn)
        .await
}

pub(super) async fn lock_device(conn: &mut PgConnection, device: Uuid) -> Result<(), Error> {
    match devices::lock_metrics_source(conn, DeviceId::from(device)).await {
        Ok(MetricsSource::Daemon) => Ok(()),
        Ok(_) | Err(devices::Error::NotFound(_)) => Err(Error::Refused("DEVICE_NOT_FOUND")),
        Err(devices::Error::Database(error)) => Err(error.into()),
        Err(_) => Err(Error::Refused("UNAVAILABLE")),
    }
}

pub(super) async fn blocked(
    conn: &mut PgConnection,
    device: Uuid,
    at: DateTime<Utc>,
) -> Result<bool, Error> {
    crate::work::jobs::blocks_dispatch(conn, DeviceId::from(device), at)
        .await
        .map_err(Error::Jobs)
}

pub(super) async fn credential_is_live(
    conn: &mut PgConnection,
    device: Uuid,
    at: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT FROM metrics.daemons WHERE device_id = $1 AND expires_at > $2)",
    )
    .bind(device)
    .bind(at)
    .fetch_one(conn)
    .await
}

pub(super) async fn launch(
    conn: &mut PgConnection,
    account: Uuid,
    id: Uuid,
) -> Result<Launch, Error> {
    sqlx::query_as("SELECT * FROM agent_threads.launches WHERE account_id = $1 AND launch_id = $2")
        .bind(account)
        .bind(id)
        .fetch_optional(conn)
        .await?
        .ok_or(Error::Refused("LAUNCH_UNKNOWN"))
}

pub(super) async fn launch_device(db: &PgPool, account: Uuid, id: Uuid) -> Result<Uuid, Error> {
    sqlx::query_scalar(
        "SELECT device_id FROM agent_threads.launches WHERE account_id = $1 AND launch_id = $2",
    )
    .bind(account)
    .bind(id)
    .fetch_optional(db)
    .await?
    .ok_or(Error::Refused("LAUNCH_UNKNOWN"))
}

pub(in crate::fleet) async fn operator(conn: &mut PgConnection) -> Result<Uuid, Error> {
    sqlx::query_scalar("SELECT id FROM access.operators")
        .fetch_optional(conn)
        .await?
        .ok_or(Error::Refused("TOKEN_REFUSED"))
}

pub(super) async fn expire_waiting(
    conn: &mut PgConnection,
    device: Uuid,
    at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE agent_threads.commands SET state = 'expired', payload = NULL, closed_at = $2
         WHERE device_id = $1 AND state = 'waiting' AND deadline <= $2",
    )
    .bind(device)
    .bind(at)
    .execute(conn)
    .await?;
    Ok(())
}

pub(super) async fn capacity(
    conn: &mut PgConnection,
    device: Uuid,
    prompt: bool,
) -> Result<(), Error> {
    let (total, prompts): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE kind = 'prompt') FROM agent_threads.commands
         WHERE device_id = $1 AND state IN ('waiting', 'delivered')",
    )
    .bind(device)
    .fetch_one(conn)
    .await?;
    if total >= 32 || (prompt && prompts >= 31) {
        return Err(Error::Refused("COMMANDS_FULL"));
    }
    Ok(())
}

pub(super) async fn fingerprint(
    conn: &mut PgConnection,
    version: i16,
    domain: &str,
    input: &impl Serialize,
) -> Result<Vec<u8>, Error> {
    let key: Vec<u8> =
        sqlx::query_scalar("SELECT key FROM agent_threads.fingerprint_keys WHERE version = $1")
            .bind(version)
            .fetch_one(conn)
            .await?;
    let key = zeroize::Zeroizing::new(key);
    let mut mac =
        ring::hmac::Context::with_key(&ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &key));
    mac.update(domain.as_bytes());
    mac.update(&[0]);
    mac.update(&zeroize::Zeroizing::new(serde_json::to_vec(input)?));
    Ok(mac.sign().as_ref().to_vec())
}

pub(super) async fn reserve_record(conn: &mut PgConnection, key: i16) -> Result<(), Error> {
    let updated = sqlx::query(
        "UPDATE agent_threads.fingerprint_keys SET records = records + 1
         WHERE version = $1 AND records < 100000",
    )
    .bind(key)
    .execute(conn)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(Error::Refused("COMMANDS_FULL"));
    }
    Ok(())
}

pub(super) async fn close_first(
    conn: &mut PgConnection,
    (owner, launch, cause): (Owner, &Launch, &str),
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE agent_threads.commands SET state = 'refused', cause = $3, payload = NULL, closed_at = clock_timestamp()
         WHERE account_id = $1 AND command_id = $2 AND state = 'waiting'",
    )
    .bind(owner.account)
    .bind(launch.first_command.to_string())
    .bind(cause)
    .execute(conn)
    .await?;
    Ok(())
}

pub(super) async fn lock_id(
    conn: &mut PgConnection,
    account: Uuid,
    id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 308031))")
        .bind(format!("{account}/{id}"))
        .execute(conn)
        .await?;
    Ok(())
}

pub(super) async fn lock_existing(conn: &mut PgConnection, device: Uuid) -> Result<(), Error> {
    match devices::lock(conn, DeviceId::from(device)).await {
        Ok(()) => Ok(()),
        Err(devices::Error::NotFound(_)) => Err(Error::Refused("THREAD_NOT_FOUND")),
        Err(devices::Error::Database(error)) => Err(error.into()),
        Err(_) => Err(Error::Refused("UNAVAILABLE")),
    }
}

pub(in crate::fleet) async fn clear_offers(
    conn: &mut PgConnection,
    device: DeviceId,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM agent_threads.start_offers WHERE device_id = $1")
        .bind(device)
        .execute(&mut *conn)
        .await?;
    sqlx::query(
        "UPDATE agent_threads.control_offers SET expires_at = clock_timestamp()
         FROM agent_threads.launches launch
         WHERE launch.account_id = control_offers.account_id
           AND launch.launch_id = control_offers.launch_id AND launch.device_id = $1",
    )
    .bind(device)
    .execute(conn)
    .await?;
    Ok(())
}

use std::collections::HashMap;

use chrono::{DateTime, NaiveTime, Utc};
use chrono_tz::Tz;
use sqlx::{PgConnection, PgPool};

use super::rules::{DevicePolicy, QuietHours};
use crate::ids::DeviceId;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("a stored device policy cannot be read")]
    Unreadable,
    #[error("no such device")]
    NoDevice,
}

type PolicyRow = (
    DeviceId,
    i16,
    i16,
    Option<NaiveTime>,
    Option<NaiveTime>,
    Option<String>,
    bool,
    bool,
);

pub(super) async fn policies(db: &PgPool) -> Result<HashMap<DeviceId, DevicePolicy>, StoreError> {
    let rows: Vec<PolicyRow> = sqlx::query_as(
        "SELECT device_id, memory_share, disk_share, quiet_start, quiet_end, quiet_zone, \
           mains_only, idle_only \
         FROM compute.device_policies",
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(read_row).collect()
}

fn read_row(row: PolicyRow) -> Result<(DeviceId, DevicePolicy), StoreError> {
    let (device, memory, disk, start, end, zone, mains_only, idle_only) = row;
    let share = |value: i16| u8::try_from(value).map_err(|_| StoreError::Unreadable);
    let quiet_hours = match (start, end, zone) {
        (Some(start), Some(end), Some(zone)) => Some(QuietHours {
            start,
            end,
            zone: zone.parse::<Tz>().map_err(|_| StoreError::Unreadable)?,
        }),
        _ => None,
    };
    let policy = DevicePolicy {
        memory_share: share(memory)?,
        disk_share: share(disk)?,
        quiet_hours,
        mains_only,
        idle_only,
    };
    Ok((device, policy))
}

pub(super) async fn write(
    conn: &mut PgConnection,
    device: DeviceId,
    policy: &DevicePolicy,
    now: DateTime<Utc>,
) -> Result<(), StoreError> {
    let hours = policy.quiet_hours;
    sqlx::query(
        "INSERT INTO compute.device_policies (device_id, memory_share, disk_share, quiet_start, \
           quiet_end, quiet_zone, mains_only, idle_only, changed_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
         ON CONFLICT (device_id) DO UPDATE SET memory_share = $2, disk_share = $3, \
           quiet_start = $4, quiet_end = $5, quiet_zone = $6, mains_only = $7, idle_only = $8, \
           changed_at = $9",
    )
    .bind(device)
    .bind(i16::from(policy.memory_share))
    .bind(i16::from(policy.disk_share))
    .bind(hours.map(|hours| hours.start))
    .bind(hours.map(|hours| hours.end))
    .bind(hours.map(|hours| hours.zone.name().to_owned()))
    .bind(policy.mains_only)
    .bind(policy.idle_only)
    .bind(now)
    .execute(conn)
    .await
    .map_err(|error| match &error {
        sqlx::Error::Database(found) if found.is_foreign_key_violation() => StoreError::NoDevice,
        _ => StoreError::Database(error),
    })?;
    Ok(())
}

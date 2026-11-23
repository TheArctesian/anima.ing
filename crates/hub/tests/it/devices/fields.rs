//! Device fields from the checks domain (CHK-004, CHK-011, API-005, API-006).

mod batching;
mod byte_order;
mod clock;
mod create;
mod ip;
mod metrics;
mod order;
mod status;
mod web_interfaces;

use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use sqlx::PgPool;

use super::harness::now;

pub const PVE_01: &str = "0190a000-0000-7000-8002-000000000001";
pub const DOCKER_HOST: &str = "0190a000-0000-7000-8002-000000000002";
pub const OLLAMA: &str = "0190a000-0000-7000-8002-000000000003";
pub const JELLYFIN: &str = "0190a000-0000-7000-8002-000000000006";
pub const TRUENAS: &str = "0190a000-0000-7000-8002-000000000007";
pub const BACKUP_02: &str = "0190a000-0000-7000-8002-000000000008";
pub const PIHOLE: &str = "0190a000-0000-7000-8002-000000000005";

pub async fn set_status(pool: &PgPool, device: &str, status: &str) {
    sqlx::query("UPDATE checks.checks SET status = $2 WHERE device_id = $1::text::uuid")
        .bind(device)
        .bind(status)
        .execute(pool)
        .await
        .unwrap();
}

pub async fn ping_uptime(
    pool: &PgPool,
    device: &str,
    (table, ago): (&str, TimeDelta),
    counts: (i32, i32),
) {
    let bucket: DateTime<Utc> = now() - ago;
    sqlx::query(&format!(
        "INSERT INTO checks.{table} (check_id, bucket, up_count, down_count) \
         SELECT id, $2, $3, $4 FROM checks.checks WHERE device_id = $1::text::uuid AND name = 'Ping'"
    ))
    .bind(device)
    .bind(bucket)
    .bind(counts.0)
    .bind(counts.1)
    .execute(pool)
    .await
    .unwrap();
}

/// The fixture sets jellyfin's manual flag, so it sorts as Maintenance whatever its status.
pub async fn seed_standing(pool: &PgPool) {
    for (device, status) in [
        (PVE_01, "down"),
        (TRUENAS, "down"),
        (DOCKER_HOST, "degraded"),
        (JELLYFIN, "down"),
        (BACKUP_02, "up"),
        (OLLAMA, "up"),
        (PIHOLE, "up"),
    ] {
        set_status(pool, device, status).await;
    }
    let hour = TimeDelta::hours(1);
    ping_uptime(pool, TRUENAS, ("uptime_minute", hour), (1, 3)).await;
    ping_uptime(pool, PVE_01, ("uptime_minute", hour), (1, 1)).await;
    ping_uptime(pool, DOCKER_HOST, ("uptime_minute", hour), (3, 1)).await;
    ping_uptime(pool, OLLAMA, ("uptime_minute", hour), (1, 0)).await;
}

pub fn names(connection: &Value) -> Vec<&str> {
    connection["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| edge["node"]["name"].as_str().unwrap())
        .collect()
}

pub const BY_STATUS: [&str; 13] = [
    "pve-01",
    "truenas",
    "docker-host",
    "home-assistant",
    "linux-pc-home",
    "linux-pc-office-2",
    "mac-office-1",
    "mac-office-2",
    "win-desktop",
    "jellyfin",
    "backup-02",
    "ollama",
    "pihole",
];

pub const BY_UPTIME: [&str; 13] = [
    "truenas",
    "pve-01",
    "docker-host",
    "ollama",
    "backup-02",
    "home-assistant",
    "jellyfin",
    "linux-pc-home",
    "linux-pc-office-2",
    "mac-office-1",
    "mac-office-2",
    "pihole",
    "win-desktop",
];

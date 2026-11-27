//! An authenticated announcement initializes only untouched private-device settings.

use axum::http::StatusCode;
use chrono::{DurationRound as _, TimeDelta};
use serde_json::json;
use sqlx::PgPool;

use super::{database_now, enrolled, push, pve};
use crate::{fleet::devices, ids::DeviceId};

async fn computer(pool: &PgPool, host: &str) -> DeviceId {
    let id = DeviceId::from(uuid::Uuid::from_u128(42));
    sqlx::query("INSERT INTO devices.devices (id, site_id, name, host, kind, os, created_at, updated_at) SELECT $1, site_id, 'test-os', $2, 'server', 'linux', created_at, updated_at FROM devices.devices WHERE id = $3")
        .bind(id).bind(host).bind(pve()).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO devices.ssh_settings (device_id, enabled, username, auth) VALUES ($1, false, '', 'private_key')")
        .bind(id).execute(pool).await.unwrap();
    id
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../tests/fixtures", scripts("devices"))
)]
async fn private_access_defaults_and_owner_overrides(pool: PgPool) {
    let id = computer(&pool, "fd41:4e49:4d41::42").await;
    let now = database_now(&pool).await;
    let token = enrolled(&pool, id, now, 42).await;
    let batch = json!({"device": id, "series": [{"kind":"cpu_percent"}], "points": [{
        "at": now.duration_trunc(TimeDelta::seconds(10)).unwrap().format("%Y-%m-%dT%H:%M:%SZ").to_string(), "mean":[1.0]
    }], "inventory": {
        "architecture":"x86_64", "os":"linux", "osVersion":"26.11", "tools":[], "buildTargets":[],
        "remoteAccess":{"sshUser":"admin", "rdpUser":"admin"}, "remoteVncUser":"admin"
    }});
    assert_eq!(
        push(&pool, now, &token, &batch).await,
        StatusCode::NO_CONTENT
    );
    let ready = devices::get(&pool, id).await.unwrap();
    assert!(ready.ssh.enabled);
    assert_eq!(ready.ssh.username, "admin");
    assert!(ready.rdp.enabled);
    assert!(ready.vnc.enabled);
    assert_eq!(ready.vnc.username.as_deref(), Some("admin"));
    assert!(!ready.rdp.ignore_certificate);
    sqlx::query("UPDATE devices.ssh_settings SET enabled = false WHERE device_id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE devices.vnc_settings SET enabled = false WHERE device_id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE devices.rdp_settings SET port = 3390 WHERE device_id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        push(&pool, now, &token, &batch).await,
        StatusCode::NO_CONTENT
    );
    let kept = devices::get(&pool, id).await.unwrap();
    assert!(!kept.ssh.enabled);
    assert_eq!(kept.rdp.port, 3390);
    assert!(!kept.vnc.enabled);
    let mut other = batch;
    other["device"] = json!(pve());
    assert_ne!(
        push(&pool, now, &token, &other).await,
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../tests/fixtures", scripts("devices"))
)]
async fn access_announcement_does_not_enable_lan_device(pool: PgPool) {
    let id = computer(&pool, "192.0.2.42").await;
    let now = database_now(&pool).await;
    let token = enrolled(&pool, id, now, 42).await;
    let batch = json!({"device": id, "series": [{"kind":"cpu_percent"}], "points": [{
        "at": now.duration_trunc(TimeDelta::seconds(10)).unwrap().format("%Y-%m-%dT%H:%M:%SZ").to_string(), "mean":[1.0]
    }], "inventory": {
        "architecture":"x86_64", "os":"linux", "osVersion":"26.11", "tools":[], "buildTargets":[],
        "remoteAccess":{"sshUser":"admin", "rdpUser":"admin"}, "remoteVncUser":"admin"
    }});
    assert_eq!(
        push(&pool, now, &token, &batch).await,
        StatusCode::NO_CONTENT
    );
    let kept = devices::get(&pool, id).await.unwrap();
    assert!(!kept.ssh.enabled);
    assert!(!kept.rdp.enabled);
    assert!(!kept.vnc.enabled);
}

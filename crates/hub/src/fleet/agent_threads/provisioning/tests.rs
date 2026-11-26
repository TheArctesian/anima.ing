use super::{delivery::poll, store};
use crate::ids::DeviceId;
use agent_threads::managed::workspace::{Action, Command, Phase, Poll, Receipt, View};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

async fn full_node(pool: &PgPool) -> (DeviceId, Command, DateTime<Utc>) {
    let device: DeviceId = "0190a000-0000-7000-8002-000000000002".parse().unwrap();
    let now = DateTime::parse_from_rfc3339("2026-11-24T12:00:00Z")
        .unwrap()
        .to_utc();
    let command = Command {
        request: Uuid::now_v7(),
        workspace: Uuid::now_v7(),
        principal: Uuid::now_v7(),
        action: Action::Prepare {
            storage_bytes: 1 << 30,
            source: None,
        },
    };
    sqlx::query(
        "INSERT INTO agent_threads.workspace_nodes(device_id,ready,seen_at) VALUES($1,true,$2)",
    )
    .bind(device)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    for index in 0..128 {
        let workspace = if index == 0 {
            command.workspace
        } else {
            Uuid::now_v7()
        };
        sqlx::query("INSERT INTO agent_threads.workspace_owners(device_id,workspace,principal,created_at) VALUES($1,$2,$3,$4)")
            .bind(device).bind(workspace).bind(command.principal).bind(now).execute(pool).await.unwrap();
    }
    (device, command, now)
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../tests/fixtures", scripts("devices"))
)]
async fn confirmed_deletion_reclaims_capacity_without_reusing_identity(pool: PgPool) {
    let (device, mut delete, now) = full_node(&pool).await;
    let mut next = delete.clone();
    next.request = Uuid::now_v7();
    next.workspace = Uuid::now_v7();
    let mut tx = pool.begin().await.unwrap();
    assert!(store::enqueue(&mut tx, device, &next, now).await.is_err());
    delete.action = Action::Delete {
        generation: 0,
        confirmed: true,
    };
    store::enqueue(&mut tx, device, &delete, now).await.unwrap();
    let request = |report| Poll {
        device: device.into(),
        ready: true,
        report,
    };
    let offered = poll(&mut tx, device, (request(None), None, false), now)
        .await
        .unwrap();
    assert_eq!(offered.command, Some(delete.clone()));
    let mut receipt = Receipt {
        command: delete.clone(),
        result: None,
        failure: Some("Storage remains.".into()),
    };
    poll(
        &mut tx,
        device,
        (request(Some(receipt.clone())), None, false),
        now,
    )
    .await
    .unwrap();
    assert!(store::enqueue(&mut tx, device, &next, now).await.is_err());
    receipt.failure = None;
    receipt.result = Some(View {
        workspace: delete.workspace,
        generation: 0,
        phase: Phase::Deleted,
        storage_bytes: 0,
        checkout: None,
    });
    poll(
        &mut tx,
        device,
        (request(Some(receipt.clone())), None, false),
        now,
    )
    .await
    .unwrap();
    poll(&mut tx, device, (request(Some(receipt)), None, false), now)
        .await
        .unwrap();
    assert!(store::enqueue(&mut tx, device, &next, now).await.unwrap());
    let mut reuse = next.clone();
    reuse.request = Uuid::now_v7();
    reuse.workspace = delete.workspace;
    assert!(store::enqueue(&mut tx, device, &reuse, now).await.is_err());
    tx.commit().await.unwrap();
    retained_history(&pool, device, &delete, &next).await;
}

async fn retained_history(pool: &PgPool, device: DeviceId, delete: &Command, next: &Command) {
    let now = DateTime::parse_from_rfc3339("2026-11-24T12:00:00Z")
        .unwrap()
        .to_utc();
    let status = store::status(pool, delete.request, delete.principal)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status["state"], "completed");
    let inventory = store::inventory(pool, device, delete.principal, now)
        .await
        .unwrap();
    let workspaces = inventory["workspaces"].as_array().unwrap();
    assert_eq!(workspaces.len(), 128);
    assert!(
        workspaces
            .iter()
            .all(|row| row["workspace"] != delete.workspace.to_string())
    );
    assert!(
        workspaces
            .iter()
            .any(|row| row["workspace"] == next.workspace.to_string())
    );
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../tests/fixtures", scripts("devices"))
)]
async fn workspace_delivery_keeps_cleanup_and_receipt_replay_available_under_pressure(
    pool: PgPool,
) {
    use agent_threads::managed::workspace::Capacity;
    use chrono::TimeDelta;

    let device: DeviceId = "0190a000-0000-7000-8002-000000000002".parse().unwrap();
    let now = Utc::now();
    let principal = Uuid::now_v7();
    let request = |report, capacity| {
        (
            Poll {
                device: device.into(),
                ready: true,
                report,
            },
            Some(capacity),
            false,
        )
    };
    let make = |action| Command {
        request: Uuid::now_v7(),
        workspace: Uuid::now_v7(),
        principal,
        action,
    };
    let prepare = make(Action::Prepare {
        storage_bytes: 1 << 30,
        source: None,
    });
    let runtime = Capacity {
        memory_bytes: 1 << 30,
        cpu_millis: 500,
        tasks: 256,
        storage_bytes: 0,
    };
    let start = make(Action::Start {
        generation: 1,
        memory_bytes: runtime.memory_bytes,
        cpu_millis: runtime.cpu_millis,
        tasks: runtime.tasks,
    });
    let stop = make(Action::Stop { generation: 1 });
    let paused = Capacity::default();
    let mut tx = pool.begin().await.unwrap();
    poll(&mut tx, device, request(None, paused), now)
        .await
        .unwrap();
    for command in [&start, &stop] {
        sqlx::query("INSERT INTO agent_threads.workspace_owners(device_id,workspace,principal,created_at) VALUES($1,$2,$3,$4)")
            .bind(device).bind(command.workspace).bind(principal).bind(now).execute(&mut *tx).await.unwrap();
    }
    for (seconds, command) in [(0, &prepare), (1, &start), (2, &stop)] {
        store::enqueue(&mut tx, device, command, now + TimeDelta::seconds(seconds))
            .await
            .unwrap();
    }
    let now = now + TimeDelta::seconds(3);
    let offered = poll(&mut tx, device, request(None, paused), now)
        .await
        .unwrap();
    assert_eq!(offered.command, Some(stop.clone()));
    let receipt = Receipt {
        result: Some(View {
            workspace: stop.workspace,
            generation: 1,
            phase: Phase::Stopped,
            storage_bytes: 1 << 30,
            checkout: None,
        }),
        command: stop,
        failure: None,
    };
    let paused_delivery = poll(&mut tx, device, request(Some(receipt), paused), now)
        .await
        .unwrap();
    assert!(paused_delivery.command.is_none());
    let offered = poll(&mut tx, device, request(None, runtime), now)
        .await
        .unwrap();
    assert_eq!(offered.command.as_ref(), Some(&start));
    let replayed = poll(&mut tx, device, request(None, paused), now)
        .await
        .unwrap();
    assert_eq!(replayed.command, Some(start));
    tx.commit().await.unwrap();

    let fresh = store::inventory(&pool, device, principal, now)
        .await
        .unwrap();
    assert_eq!(fresh["available"], true);
    assert_eq!(fresh["capacity"], serde_json::to_value(paused).unwrap());
    let stale = store::inventory(&pool, device, principal, now + TimeDelta::seconds(31))
        .await
        .unwrap();
    assert_eq!(stale["available"], false);
    assert!(stale["capacity"].is_null());
}

use super::{
    Fixture, Import, Method, Offer, PERMITTED, POLL, PgPool, StatusCode, call, contents, report,
    send, transfer_state,
};
use anima_hub::clock::Clock;
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) const OTHER: &str = "0190a000-0000-7000-8002-000000000003";

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../fixtures", scripts("devices"))
)]
async fn automatic_move_waits_for_headroom_and_stays_bound_to_the_selected_node(pool: PgPool) {
    let fixture = Fixture::automatic(&pool).await;
    let source = fixture.receipt.source;
    let other = fixture.daemons.token_for(OTHER).await;
    capacity(
        &fixture,
        &fixture.destination,
        source.binding.destination,
        127,
    )
    .await;
    capacity(&fixture, &other, OTHER.parse().unwrap(), 512).await;
    sqlx::query(
        "INSERT INTO jobs.machines(device_id,switch,unclean,excluded,last_claim_at)
                 VALUES($1,'on',false,true,$2)",
    )
    .bind(OTHER.parse::<Uuid>().unwrap())
    .bind(fixture.daemons.clock.now())
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(poll_source(&fixture).await.status(), StatusCode::NO_CONTENT);
    let waiting = transfer_state(&fixture).await;
    assert_eq!(waiting["state"], "waiting-placement");
    assert!(waiting["destinationDevice"].is_null());
    assert!(enabled(&fixture).await);

    capacity(
        &fixture,
        &fixture.destination,
        source.binding.destination,
        128,
    )
    .await;
    let destination_poll = send(
        &fixture.app,
        call(Method::POST, POLL, &fixture.destination),
        PERMITTED,
    )
    .await;
    assert_eq!(destination_poll.status(), StatusCode::NO_CONTENT);
    capacity(
        &fixture,
        &fixture.destination,
        source.binding.destination,
        127,
    )
    .await;
    assert_eq!(poll_source(&fixture).await.status(), StatusCode::NO_CONTENT);
    assert_eq!(transfer_state(&fixture).await["state"], "waiting-placement");
    assert!(enabled(&fixture).await);

    capacity(
        &fixture,
        &fixture.destination,
        source.binding.destination,
        128,
    )
    .await;
    let mut destination_update = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM devices.devices WHERE id=$1 FOR UPDATE")
        .bind(source.binding.destination)
        .execute(&mut *destination_update)
        .await
        .unwrap();
    let busy = tokio::time::timeout(std::time::Duration::from_secs(5), poll_source(&fixture))
        .await
        .expect("source poll waited for the destination update");
    assert_eq!(busy.status(), StatusCode::NO_CONTENT);
    assert!(enabled(&fixture).await);
    destination_update.rollback().await.unwrap();

    let first = poll_source(&fixture).await;
    assert_eq!(first.status(), StatusCode::OK);
    let first = contents(first).await;
    let offer: Offer = serde_json::from_slice(&first).unwrap();
    assert!(matches!(offer, Offer::Export { source: planned, .. } if planned == source));
    let placed = transfer_state(&fixture).await;
    assert_eq!(
        placed["destinationDevice"],
        source.binding.destination.to_string()
    );
    assert_eq!(placed["state"], "awaiting-export");
    assert!(!enabled(&fixture).await);

    sqlx::query("UPDATE jobs.machines SET excluded=false WHERE device_id=$1")
        .bind(OTHER.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let replay = submit(&fixture, super::fixture::automatic_action(source)).await;
    assert_eq!(
        replay["data"]["queueManagedWorkspaceRequest"],
        source.binding.transfer.to_string()
    );
    let changed = submit(
        &fixture,
        json!({"kind":"move","generation":1,"storage_bytes":1_u64<<30,
        "destination_device":OTHER}),
    )
    .await;
    assert!(changed["errors"].is_array());
    capacity(
        &fixture,
        &fixture.destination,
        source.binding.destination,
        127,
    )
    .await;
    let replay = poll_source(&fixture).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(contents(replay).await, first);

    capacity(
        &fixture,
        &fixture.destination,
        source.binding.destination,
        128,
    )
    .await;
    let import: Import = super::withdrawal::activation_offer(&fixture).await;
    assert_eq!(
        report(&fixture, "activated", &import, &fixture.destination).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(transfer_state(&fixture).await["state"], "activated");
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../fixtures", scripts("devices"))
)]
async fn withdrawing_an_unplaced_move_never_fences_the_source(pool: PgPool) {
    let fixture = Fixture::automatic(&pool).await;
    let transfer = fixture.receipt.source.binding.transfer;
    let cancelled = submit(&fixture, json!({"kind":"withdraw-move"})).await;
    assert_eq!(
        cancelled["data"]["queueManagedWorkspaceRequest"],
        transfer.to_string()
    );
    assert_eq!(poll_source(&fixture).await.status(), StatusCode::NO_CONTENT);
    let state = transfer_state(&fixture).await;
    assert_eq!(state["state"], "withdrawn");
    assert!(state["destinationDevice"].is_null());
    assert!(enabled(&fixture).await);
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../fixtures", scripts("devices"))
)]
async fn placement_expiry_leaves_the_source_running_and_allows_a_new_request(pool: PgPool) {
    let fixture = Fixture::automatic(&pool).await;
    fixture.daemons.clock.advance(chrono::TimeDelta::hours(1));
    let device: Uuid = crate::daemon::harness::PVE_01.parse().unwrap();
    let mut source = fixture.receipt.source;
    capacity(&fixture, &fixture.source, device, 512).await;
    capacity(
        &fixture,
        &fixture.destination,
        source.binding.destination,
        512,
    )
    .await;
    assert_eq!(poll_source(&fixture).await.status(), StatusCode::NO_CONTENT);
    assert_eq!(transfer_state(&fixture).await["state"], "placement-expired");
    assert!(enabled(&fixture).await);
    source.binding.transfer = Uuid::now_v7();
    let next = super::fixture::submit(
        &fixture.app,
        &fixture.daemons,
        source,
        super::fixture::automatic_action(source),
    )
    .await;
    assert_eq!(
        next["data"]["queueManagedWorkspaceRequest"],
        source.binding.transfer.to_string()
    );
}

pub(super) async fn poll_source(fixture: &Fixture) -> axum::response::Response {
    send(
        &fixture.app,
        call(Method::POST, POLL, &fixture.source),
        PERMITTED,
    )
    .await
}

pub(super) async fn capacity(fixture: &Fixture, token: &str, device: Uuid, tasks: u64) {
    let mut request = call(Method::POST, "/daemon/v2/workspaces/poll", token);
    *request.body_mut() = axum::body::Body::from(json!({"device":device,"ready":true,"report":null,
        "capacity":{"storageBytes":2_u64<<30,"memoryBytes":2_u64<<30,"cpuMillis":1000,"tasks":tasks}}).to_string());
    let response = send(&fixture.app, request, PERMITTED).await;
    let status = response.status();
    assert_eq!(status, StatusCode::OK, "{:?}", contents(response).await);
}

pub(super) async fn enabled(fixture: &Fixture) -> bool {
    let answer = fixture
        .daemons
        .operate(
            "query($device:ID!){managedCodeWorkspaces(device:$device)}",
            json!({"device":crate::daemon::harness::PVE_01}),
        )
        .await;
    answer["data"]["managedCodeWorkspaces"][0]["enabled"]
        .as_bool()
        .unwrap()
}

async fn submit(fixture: &Fixture, action: Value) -> Value {
    super::fixture::submit(
        &fixture.app,
        &fixture.daemons,
        fixture.receipt.source,
        action,
    )
    .await
}

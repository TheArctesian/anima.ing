use super::{
    Body, Fixture, Import, Method, Offer, PERMITTED, POLL, PgPool, StatusCode, call, contents,
    report, send, transfer_state,
};
use crate::daemon::harness::{DOCKER_HOST, PVE_01};
use agent_threads::managed::workspace::handoff::Source;
use anima_hub::clock::Clock as _;
use axum::http::{HeaderValue, header};
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) async fn request(fixture: &Fixture, device: &str, confirmed: bool) -> Value {
    command(
        fixture,
        device,
        fixture.receipt.source.binding.transfer,
        json!({"kind":"retire-source","confirmed":confirmed}),
    )
    .await
}

pub(super) async fn activated_move_can_retire_and_return(
    fixture: &Fixture,
    import: &Import,
    pool: &PgPool,
) {
    let source = import.receipt.source;
    assert!(request(fixture, PVE_01, true).await["errors"].is_array());
    assert!(request(fixture, DOCKER_HOST, false).await["errors"].is_array());
    assert_eq!(
        report(fixture, "retired", &source, &fixture.source).await,
        StatusCode::CONFLICT
    );

    let generation = start_destination_generation(fixture, import, pool).await;
    let returning = Uuid::now_v7();
    let move_back = json!({"kind":"move","destination_device":PVE_01,"generation":generation,"storage_bytes":import.storage_bytes});
    assert!(command(fixture, DOCKER_HOST, returning, move_back.clone()).await["errors"].is_array());

    require_retirement_offer(fixture, source).await;
    confirm_retirement(fixture, source, generation).await;
    assert_eq!(
        command(fixture, DOCKER_HOST, returning, move_back).await["data"]["queueManagedWorkspaceRequest"],
        returning.to_string()
    );
}

async fn require_retirement_offer(fixture: &Fixture, source: Source) {
    let id = source.binding.transfer;
    for _ in 0..2 {
        assert_eq!(
            request(fixture, DOCKER_HOST, true).await["data"]["queueManagedWorkspaceRequest"],
            id.to_string()
        );
    }
    let copies = retained(fixture).await;
    assert_eq!(copies[0]["transfer"], id.to_string());
    assert_eq!(copies[0]["sourceDevice"], PVE_01);
    assert_eq!(copies[0]["pending"], true);
    let offer = send(
        &fixture.app,
        call(Method::POST, POLL, &fixture.source),
        PERMITTED,
    )
    .await;
    let offer: Offer = serde_json::from_slice(&contents(offer).await).unwrap();
    assert!(matches!(offer, Offer::Retire { source: bound, .. } if bound == source));
}

async fn confirm_retirement(fixture: &Fixture, source: Source, generation: u64) {
    assert_eq!(
        report(fixture, "retired", &source, &fixture.destination).await,
        StatusCode::CONFLICT
    );
    let newer = Source {
        generation,
        ..source
    };
    assert_eq!(
        report(fixture, "retired", &newer, &fixture.source).await,
        StatusCode::CONFLICT
    );
    assert_eq!(transfer_state(fixture).await["sourceRetired"], false);
    for _ in 0..2 {
        assert_eq!(
            report(fixture, "retired", &source, &fixture.source).await,
            StatusCode::NO_CONTENT
        );
    }
    assert_eq!(transfer_state(fixture).await["sourceRetired"], true);
    assert_eq!(retained(fixture).await, json!([]));
}

async fn retained(fixture: &Fixture) -> Value {
    let result = fixture
        .daemons
        .operate(
            "query($device:ID!){managedNodeWorkspaces(device:$device)}",
            json!({"device":DOCKER_HOST}),
        )
        .await;
    result["data"]["managedNodeWorkspaces"]["retainedSources"].clone()
}

async fn start_destination_generation(fixture: &Fixture, import: &Import, pool: &PgPool) -> u64 {
    let source = import.receipt.source;
    let generation = source.generation + 1;
    let now = fixture.daemons.clock.now();
    let command = json!({"action":{"kind":"start","generation":generation}});
    let receipt = json!({"result":{"workspace":source.workspace,"generation":generation,
       "phase":"running","storageBytes":import.storage_bytes}});
    sqlx::query(
        "INSERT INTO agent_threads.workspace_requests
        (request,device_id,workspace,principal,payload,state,receipt,created_at,expires_at)
        VALUES ($1,$2,$3,$4,$5,'completed',$6,$7,$7)",
    )
    .bind(Uuid::now_v7())
    .bind(import.destination_device)
    .bind(source.workspace)
    .bind(source.principal)
    .bind(command)
    .bind(receipt)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    generation
}

async fn command(fixture: &Fixture, device: &str, request: Uuid, action: Value) -> Value {
    let mut post = call(Method::POST, "/graphql", &fixture.daemons.operator);
    post.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    *post.body_mut() = Body::from(json!({
        "query":"mutation($device:ID!, $input:ManagedWorkspaceInput!){queueManagedWorkspaceRequest(device:$device,input:$input)}",
        "variables":{"device":device,"input":{"request":request,"workspace":fixture.receipt.source.workspace,"action":action}}
    }).to_string());
    let response = send(&fixture.app, post, PERMITTED).await;
    serde_json::from_slice(&contents(response).await).unwrap()
}

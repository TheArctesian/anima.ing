use super::{
    BYTES, Body, Fixture, Import, Method, Offer, PERMITTED, POLL, PgPool, StatusCode, call,
    contents, report, send, transfer_state, upload,
};
use crate::daemon::harness::{DOCKER_HOST, PVE_01};
use agent_threads::managed::workspace::{Phase, View, handoff::Withdrawal};
use axum::http::{HeaderValue, header};
use serde_json::{Value, json};

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../fixtures", scripts("devices"))
)]
async fn withdrawal_releases_the_source_only_after_the_destination_seals_the_import(pool: PgPool) {
    let fixture = Fixture::new(&pool).await;
    let import = activation_offer(&fixture).await;
    request_withdrawal(&fixture).await;
    assert!(
        matches!(poll(&fixture, &fixture.destination).await, Some(Offer::Discard { import: bound, .. }) if bound == import)
    );
    assert!(poll(&fixture, &fixture.source).await.is_none());
    let source = import.receipt.source;
    let withdrawn = Withdrawal::Withdrawn {
        source,
        stopped: Some(View {
            workspace: source.workspace,
            generation: source.generation,
            phase: Phase::Stopped,
            storage_bytes: import.storage_bytes,
            checkout: None,
        }),
    };
    assert_eq!(
        report(&fixture, "withdrawal", &withdrawn, &fixture.source).await,
        StatusCode::CONFLICT
    );
    let discarded = Withdrawal::Discarded {
        import: import.clone(),
    };
    assert_eq!(
        report(&fixture, "withdrawal", &discarded, &fixture.source).await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        report(&fixture, "withdrawal", &discarded, &fixture.destination).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        report(&fixture, "activated", &import, &fixture.destination).await,
        StatusCode::CONFLICT
    );
    assert!(
        matches!(poll(&fixture, &fixture.source).await, Some(Offer::Withdraw { source: bound, .. }) if bound == source)
    );
    for _ in 0..2 {
        assert_eq!(
            report(&fixture, "withdrawal", &withdrawn, &fixture.source).await,
            StatusCode::NO_CONTENT
        );
    }
    let denied = super::retirement::request(&fixture, DOCKER_HOST, true).await;
    assert_eq!(denied["errors"][0]["extensions"]["code"], "CONFLICT");
    assert!(poll(&fixture, &fixture.source).await.is_none());
    let state = transfer_state(&fixture).await;
    assert_eq!(state["state"], "withdrawn");
    assert_eq!(state["sourceRetired"], false);
    let query = "query($device: ID!) { managedNodeWorkspaces(device: $device) }";
    let old = fixture
        .daemons
        .operate(query, json!({"device":import.source_device}))
        .await;
    let view = &old["data"]["managedNodeWorkspaces"]["workspaces"][0]["receipt"]["result"];
    assert_eq!(view["phase"], "stopped");
    assert_eq!(view["generation"], source.generation);
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../fixtures", scripts("devices"))
)]
async fn withdrawal_accepts_an_already_committed_activation_and_keeps_the_source_fenced(
    pool: PgPool,
) {
    let fixture = Fixture::new(&pool).await;
    let import = activation_offer(&fixture).await;
    request_withdrawal(&fixture).await;
    assert_eq!(
        report(&fixture, "activated", &import, &fixture.destination).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(transfer_state(&fixture).await["state"], "activated");
    assert!(poll(&fixture, &fixture.source).await.is_none());
    assert_eq!(
        report(
            &fixture,
            "withdrawal",
            &Withdrawal::Withdrawn {
                source: import.receipt.source,
                stopped: None
            },
            &fixture.source,
        )
        .await,
        StatusCode::CONFLICT
    );
    super::retirement::activated_move_can_retire_and_return(&fixture, &import, &pool).await;
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../fixtures", scripts("devices"))
)]
async fn withdrawal_recovers_the_source_while_an_unactivated_destination_is_offline(pool: PgPool) {
    let fixture = Fixture::new(&pool).await;
    let import = restore_offer(&fixture).await;
    fixture.daemons.clock.advance(chrono::TimeDelta::hours(1));
    request_withdrawal(&fixture).await;

    let source = import.receipt.source;
    assert!(
        matches!(poll(&fixture, &fixture.source).await, Some(Offer::Withdraw { source: bound, .. }) if bound == source)
    );
    let result = Withdrawal::Withdrawn {
        source,
        stopped: None,
    };
    assert_eq!(
        report(&fixture, "withdrawal", &result, &fixture.source).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(transfer_state(&fixture).await["state"], "withdrawn");

    assert!(
        matches!(poll(&fixture, &fixture.destination).await, Some(Offer::Discard { import: bound, .. }) if bound == import)
    );
    let discarded = Withdrawal::Discarded { import };
    assert_eq!(
        report(&fixture, "withdrawal", &discarded, &fixture.destination).await,
        StatusCode::NO_CONTENT
    );
    assert!(poll(&fixture, &fixture.destination).await.is_none());
}

pub(super) async fn activation_offer(fixture: &Fixture) -> Import {
    let import = restore_offer(fixture).await;
    assert_eq!(
        report(fixture, "restored", &import, &fixture.destination).await,
        StatusCode::NO_CONTENT
    );
    assert!(
        matches!(poll(fixture, &fixture.destination).await, Some(Offer::Activate { import: bound, .. }) if bound == import)
    );
    import
}

async fn restore_offer(fixture: &Fixture) -> Import {
    assert_eq!(
        upload(fixture, Body::from(BYTES.as_slice())).await.status(),
        StatusCode::ACCEPTED
    );
    let path = format!(
        "/daemon/v1/workspace-handoffs/{}/confirm",
        fixture.receipt.source.binding.transfer
    );
    let mut confirm = call(Method::POST, &path, &fixture.source);
    *confirm.body_mut() = Body::from(serde_json::to_vec(&fixture.receipt).unwrap());
    assert_eq!(
        send(&fixture.app, confirm, PERMITTED).await.status(),
        StatusCode::NO_CONTENT
    );
    let Some(Offer::Restore { import, .. }) = poll(fixture, &fixture.destination).await else {
        panic!("the confirmed recipient must be able to restore");
    };
    import
}

async fn request_withdrawal(fixture: &Fixture) {
    let source = fixture.receipt.source;
    let mut request = call(Method::POST, "/graphql", &fixture.daemons.operator);
    request.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    *request.body_mut() = Body::from(json!({
        "query":"mutation($device:ID!, $input:ManagedWorkspaceInput!){queueManagedWorkspaceRequest(device:$device,input:$input)}",
        "variables":{"device":PVE_01,"input":{
            "request":source.binding.transfer,"workspace":source.workspace,"action":{"kind":"withdraw-move"}
        }}
    }).to_string());
    let response = send(&fixture.app, request, PERMITTED).await;
    let answer: Value = serde_json::from_slice(&contents(response).await).unwrap();
    assert_eq!(
        answer["data"]["queueManagedWorkspaceRequest"],
        source.binding.transfer.to_string(),
        "{answer}"
    );
    assert_eq!(transfer_state(fixture).await["state"], "withdrawing");
}

async fn poll(fixture: &Fixture, token: &str) -> Option<Offer> {
    let result = send(&fixture.app, call(Method::POST, POLL, token), PERMITTED).await;
    if result.status() == StatusCode::NO_CONTENT {
        return None;
    }
    assert_eq!(result.status(), StatusCode::OK);
    Some(serde_json::from_slice(&contents(result).await).unwrap())
}

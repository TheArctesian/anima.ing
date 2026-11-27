mod continuation;
mod fixture;
mod placement;
mod retirement;
mod withdrawal;

use std::{sync::Arc, time::Duration};

use agent_threads::managed::workspace::handoff::{Import, Offer, Receipt};
use axum::{
    Router,
    body::{Body, Bytes},
    extract::ConnectInfo,
    http::{Method, Request, StatusCode, header},
    response::Response,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::TimeDelta;
use http_body_util::BodyExt as _;
use sqlx::PgPool;
use tokio::sync::oneshot;
use tower::ServiceExt as _;

use crate::daemon::harness::{OUTSIDE, PERMITTED};
use fixture::Fixture;

const BYTES: [u8; 1536] = [b'z'; 1536];
const POLL: &str = "/daemon/v1/workspace-handoffs/poll";

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn only_the_confirmed_recipient_can_download_a_workspace_checkpoint(pool: PgPool) {
    let fixture = Fixture::new(&pool).await;
    let path = &fixture.checkpoint;
    let state = upload(&fixture, Body::from(BYTES.as_slice())).await;
    assert_eq!(state.status(), StatusCode::ACCEPTED);
    let accepted: Receipt = serde_json::from_slice(&contents(state).await).unwrap();
    assert_eq!(accepted, fixture.receipt);

    for token in [&fixture.source, &fixture.destination] {
        let response = send(&fixture.app, call(Method::GET, path, token), PERMITTED).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
    let confirm = format!(
        "/daemon/v1/workspace-handoffs/{}/confirm",
        fixture.receipt.source.binding.transfer
    );
    let mut request = call(Method::POST, &confirm, &fixture.source);
    *request.body_mut() = Body::from(serde_json::to_vec(&fixture.receipt).unwrap());
    let response = send(&fixture.app, request, PERMITTED).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let other_device = send(
        &fixture.app,
        call(Method::GET, path, &fixture.source),
        PERMITTED,
    )
    .await;
    assert_eq!(other_device.status(), StatusCode::CONFLICT);
    let outside = send(
        &fixture.app,
        call(Method::GET, path, &fixture.destination),
        OUTSIDE,
    )
    .await;
    assert_eq!(outside.status(), StatusCode::FORBIDDEN);
    let allowed = send(
        &fixture.app,
        call(Method::GET, path, &fixture.destination),
        PERMITTED,
    )
    .await;
    assert_eq!(allowed.status(), StatusCode::OK);
    assert_eq!(
        allowed.headers()[header::CACHE_CONTROL],
        "private, no-store"
    );
    assert_eq!(contents(allowed).await.as_ref(), BYTES);
    report_restoration(&fixture).await;
}

async fn report_restoration(fixture: &Fixture) {
    let poll = call(Method::POST, POLL, &fixture.destination);
    let reply = send(&fixture.app, poll, PERMITTED).await;
    assert_eq!(reply.status(), StatusCode::OK);
    let offer: Offer = serde_json::from_slice(&contents(reply).await).unwrap();
    let Offer::Restore { import, .. } = offer else {
        panic!("expected the destination's restore offer");
    };
    assert_eq!(
        report(fixture, "activated", &import, &fixture.destination).await,
        StatusCode::CONFLICT
    );
    for (token, expected) in [
        (&fixture.source, StatusCode::CONFLICT),
        (&fixture.destination, StatusCode::NO_CONTENT),
        (&fixture.destination, StatusCode::NO_CONTENT),
    ] {
        assert_eq!(report(fixture, "restored", &import, token).await, expected);
    }
    let read = call(Method::GET, &fixture.checkpoint, &fixture.destination);
    assert_eq!(
        send(&fixture.app, read, PERMITTED).await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(transfer_state(fixture).await["state"], "restored");
    confirm_activation_after_lost_reply(fixture, &import).await;
}

async fn confirm_activation_after_lost_reply(fixture: &Fixture, import: &Import) {
    let poll = call(Method::POST, POLL, &fixture.destination);
    let reply = send(&fixture.app, poll, PERMITTED).await;
    let offer: Offer = serde_json::from_slice(&contents(reply).await).unwrap();
    assert!(matches!(offer, Offer::Activate { import: bound, .. } if &bound == import));

    fixture.daemons.clock.advance(TimeDelta::hours(1));
    let poll = call(Method::POST, POLL, &fixture.destination);
    let reply = send(&fixture.app, poll, PERMITTED).await;
    let offer: Offer = serde_json::from_slice(&contents(reply).await).unwrap();
    assert!(matches!(offer, Offer::InspectActivation { import: bound, .. } if &bound == import));
    verify_activation_reports(fixture, import).await;
    verify_activated_workspace(fixture, import).await;
}

async fn verify_activation_reports(fixture: &Fixture, import: &Import) {
    assert_eq!(
        report(fixture, "activated", import, &fixture.source).await,
        StatusCode::CONFLICT
    );
    for _ in 0..2 {
        assert_eq!(
            report(fixture, "activated", import, &fixture.destination).await,
            StatusCode::NO_CONTENT
        );
    }
    assert_eq!(transfer_state(fixture).await["state"], "activated");
}

async fn verify_activated_workspace(fixture: &Fixture, import: &Import) {
    let query = "query($device: ID!) { managedNodeWorkspaces(device: $device) }";
    let destination = fixture
        .daemons
        .operate(
            query,
            serde_json::json!({"device":import.destination_device}),
        )
        .await;
    let rows = destination["data"]["managedNodeWorkspaces"]["workspaces"]
        .as_array()
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0]["workspace"],
        import.receipt.source.workspace.to_string()
    );
    assert_eq!(rows[0]["receipt"]["result"]["phase"], "prepared");
    assert_eq!(
        rows[0]["receipt"]["result"]["generation"],
        import.receipt.source.generation
    );
    let source = fixture
        .daemons
        .operate(query, serde_json::json!({"device":import.source_device}))
        .await;
    assert_eq!(
        source["data"]["managedNodeWorkspaces"]["workspaces"],
        serde_json::json!([])
    );
}

async fn report(
    fixture: &Fixture,
    transition: &str,
    payload: &impl serde::Serialize,
    token: &str,
) -> StatusCode {
    let transfer = fixture.receipt.source.binding.transfer;
    let path = format!("/daemon/v1/workspace-handoffs/{transfer}/{transition}");
    let mut report = call(Method::POST, &path, token);
    *report.body_mut() = Body::from(serde_json::to_vec(payload).unwrap());
    send(&fixture.app, report, PERMITTED).await.status()
}

async fn transfer_state(fixture: &Fixture) -> serde_json::Value {
    let transfer = fixture.receipt.source.binding.transfer;
    let read = call(
        Method::GET,
        &format!("/workspaces/v1/handoffs/{transfer}"),
        &fixture.daemons.operator,
    );
    serde_json::from_slice(&contents(send(&fixture.app, read, PERMITTED).await).await).unwrap()
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn workspace_checkpoint_expires_mid_upload_without_holding_the_source_turn(pool: PgPool) {
    let fixture = Arc::new(Fixture::new(&pool).await);
    let (reading, began) = oneshot::channel();
    let (release, held) = oneshot::channel();
    let body = Body::from_stream(futures_util::stream::once(async move {
        reading.send(()).unwrap();
        held.await.unwrap();
        Ok::<_, std::io::Error>(Bytes::from_static(&BYTES))
    }));
    let waiting = fixture.clone();
    let transfer = tokio::spawn(async move { upload(&waiting, body).await });
    tokio::time::timeout(Duration::from_secs(5), began)
        .await
        .unwrap()
        .unwrap();

    let poll = call(Method::POST, POLL, &fixture.source);
    let available =
        tokio::time::timeout(Duration::from_secs(5), send(&fixture.app, poll, PERMITTED))
            .await
            .unwrap();
    assert_eq!(available.status(), StatusCode::OK);
    fixture.daemons.clock.advance(TimeDelta::hours(1));
    release.send(()).unwrap();
    let refused = tokio::time::timeout(Duration::from_secs(5), transfer)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(refused.status(), StatusCode::CONFLICT);
    assert!(!fixture.private_checkpoint().exists());
}

fn call(method: Method, path: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

async fn upload(fixture: &Fixture, body: Body) -> Response {
    let mut request = call(Method::POST, &fixture.checkpoint, &fixture.source);
    let receipt = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&fixture.receipt).unwrap());
    request
        .headers_mut()
        .insert("anima-workspace-checkpoint", receipt.parse().unwrap());
    *request.body_mut() = body;
    send(&fixture.app, request, PERMITTED).await
}

async fn send(app: &Router, mut request: Request<Body>, from: &str) -> Response {
    let peer: std::net::SocketAddr = format!("{from}:50000").parse().unwrap();
    request.extensions_mut().insert(ConnectInfo(peer));
    app.clone().oneshot(request).await.unwrap()
}

async fn contents(response: Response) -> Bytes {
    response.into_body().collect().await.unwrap().to_bytes()
}

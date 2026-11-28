use axum::{Json, Router, http::StatusCode, routing::post};
use daemon_core::wire::DaemonToken;
use serde_json::json;
use uuid::Uuid;

use crate::{
    hub::{HubClient, answer::Answer},
    remote_code::wire::Granted,
};

use super::*;

fn request() -> DispatchRequest {
    DispatchRequest {
        device: Uuid::from_u128(1),
        launch: Uuid::from_u128(2),
        work: Uuid::from_u128(3),
        attempt: 1,
        runner: Uuid::from_u128(4),
    }
}

fn granted() -> DispatchReply {
    DispatchReply::Dispatch {
        decision: DispatchDecision::Authorized {
            grant: Granted {
                authorization: Uuid::from_u128(5),
                valid_until: chrono::Utc::now(),
                remaining_ms: 10_000,
            },
        },
    }
}

#[test]
fn a_grant_from_another_exchange_phase_or_connection_never_authorizes_dispatch() {
    let wanted = || Pending::begin(7, Phase::Authorize, request()).unwrap();
    let (_, ask) = wanted();
    let (pending, _) = wanted();
    assert!(pending.receive(7, ask.reply(granted())).is_none());

    let (pending, ask) = wanted();
    assert!(pending.receive(8, ask.reply(granted())).is_none());

    let (pending, ask) = Pending::begin(7, Phase::Prepare, request()).unwrap();
    assert!(pending.receive(7, ask.reply(granted())).is_none());

    let (pending, ask) = wanted();
    assert!(matches!(
        pending.receive(7, ask.reply(granted())),
        Some(Matched::Authorized(_))
    ));
}

#[tokio::test]
async fn final_authorization_reference_from_a_synthetic_hub_stays_reconciliation_only() {
    let id = Uuid::from_u128(5);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let hub = Router::new().route(
        "/work/v1/thread-dispatch",
        post(move |Json(body): Json<DispatchRequest>| async move {
            assert!(body == request());
            (
                StatusCode::CONFLICT,
                Json(json!({"error": "NOT_HOLDER", "authorization": id})),
            )
        }),
    );
    let serving = tokio::spawn(async { axum::serve(listener, hub).await.unwrap() });
    let hub = HubClient::new(url.parse().unwrap(), std::time::Duration::from_secs(2)).unwrap();
    let token = DaemonToken::from_random([1; 32]);
    let (pending, ask) = Pending::begin(7, Phase::Authorize, request()).unwrap();
    let Answer::Accepted(decision) = hub.authorize_thread(&token, ask.request()).await else {
        panic!("hub reply did not decode")
    };
    let resolved = pending.receive(7, ask.reply(DispatchReply::Dispatch { decision }));
    assert!(
        matches!(resolved, Some(Matched::Final { authorization: Some(received) }) if received == id)
    );
    serving.abort();
}

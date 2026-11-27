use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::json;

use super::*;

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../tests/fixtures", scripts("devices"))
)]
async fn each_managed_poll_accepts_only_its_own_request_format(pool: PgPool) {
    let now = database_now(&pool).await;
    let token = enrolled_pve(&pool, now, 39).await;
    let router = daemon_router_within(
        (pool, Bus::new(16), Arc::new(FixedClock(now))),
        Deadlines::HUB,
    );
    let code = json!({"device": pve(), "workspaces": [], "reports": []});
    let server_poll = json!({"poll": code, "servers": []});
    let workspace = json!({"device": pve(), "ready": true, "report": null});

    let capacity = json!({"device": pve(), "ready": true, "report": null,
        "capacity": {"storageBytes": 0, "memoryBytes": 0, "cpuMillis": 0, "tasks": 0}});

    for (path, accepted, refused) in [
        ("/daemon/v1/code/poll", &code, &server_poll),
        ("/daemon/v2/code/poll", &server_poll, &code),
        ("/daemon/v1/workspaces/poll", &workspace, &capacity),
        ("/daemon/v2/workspaces/poll", &capacity, &workspace),
    ] {
        let (status, body) = poll(&router, path, &token, accepted).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(
            body,
            json!({"command": null, "valid_until": null, "acknowledged": []})
        );
        let (status, body) = poll(&router, path, &token, refused).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(body, json!({"error": "MALFORMED"}));
    }
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../tests/fixtures", scripts("devices"))
)]
async fn a_listener_is_bound_to_one_ready_generation_and_legacy_poll_clears_it(pool: PgPool) {
    let now = database_now(&pool).await;
    let token = enrolled_pve(&pool, now, 39).await;
    let router = daemon_router_within(
        (pool.clone(), Bus::new(16), Arc::new(FixedClock(now))),
        Deadlines::HUB,
    );
    let workspace = uuid::Uuid::from_u128(7);
    let inventory = json!({
        "device": pve(), "reports": [],
        "workspaces": [{"workspace": workspace, "generation": 7, "ready": true}]
    });
    let v2 = "/daemon/v2/code/poll";
    let mut observed = json!({
        "poll": inventory,
        "servers": [{"workspace": workspace, "generation": 7, "ports": [3000]}]
    });
    assert_eq!(poll(&router, v2, &token, &observed).await.0, StatusCode::OK);
    assert_eq!(stored_ports(&pool, workspace).await, [3000]);

    observed["servers"][0]["generation"] = json!(6);
    assert_eq!(
        poll(&router, v2, &token, &observed).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(stored_ports(&pool, workspace).await, [3000]);

    let legacy = poll(&router, "/daemon/v1/code/poll", &token, &inventory).await;
    assert_eq!(legacy.0, StatusCode::OK);
    assert_eq!(stored_ports(&pool, workspace).await, Vec::<i32>::new());
}

async fn stored_ports(pool: &PgPool, workspace: uuid::Uuid) -> Vec<i32> {
    sqlx::query_scalar(
        "SELECT listening_ports FROM agent_threads.workspaces WHERE device_id=$1 AND workspace=$2",
    )
    .bind(pve())
    .bind(workspace)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn poll(
    router: &Router,
    path: &str,
    token: &DaemonToken,
    body: &Value,
) -> (StatusCode, Value) {
    let mut request = Request::post(path)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token.expose()))
        .body(Body::from(body.to_string()))
        .unwrap();
    request
        .extensions_mut()
        .insert(ClientAddr("100.64.0.7".parse().unwrap()));
    let response = router.clone().oneshot(request).await.unwrap();
    let (parts, body) = response.into_parts();
    let bytes = to_bytes(body, 4096).await.unwrap();
    (parts.status, serde_json::from_slice(&bytes).unwrap())
}

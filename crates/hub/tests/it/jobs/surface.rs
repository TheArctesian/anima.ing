use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use super::{PVE_01, TEST_PROJECT, Work, build, claim_of, runner};

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn r_270_allowlist_replacement_changes_secret_bearing_placement(pool: PgPool) {
    let work = Work::new(&pool).await;
    let token = work.daemons.token_for(PVE_01).await;
    let runner = work.enqueue(runner()).await;
    let change = "mutation($project: ID!, $devices: [ID!]!) {
        setProjectWorkDevices(projectId: $project, deviceIds: $devices)
    }";
    let clear = work
        .daemons
        .operate(change, json!({"project": TEST_PROJECT, "devices": []}))
        .await;
    assert_eq!(clear["data"]["setProjectWorkDevices"], true, "{clear}");
    let claim = claim_of(PVE_01, &json!([]));
    assert_eq!(work.claim(&token, &claim).await.0, StatusCode::NO_CONTENT);
    let allow = work
        .daemons
        .operate(
            change,
            json!({"project": TEST_PROJECT, "devices": [PVE_01]}),
        )
        .await;
    assert_eq!(allow["data"]["setProjectWorkDevices"], true, "{allow}");
    let (status, chosen) = work.claim(&token, &claim).await;
    assert_eq!(status, StatusCode::OK, "{chosen}");
    assert_eq!(chosen["id"], runner.id.to_string());
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn r_270_list_cursor_and_stop_preserve_job_identity(pool: PgPool) {
    let work = Work::new(&pool).await;
    let one = work.enqueue(build()).await;
    let two = work.enqueue(build()).await;
    let page = "query($after: String) {
        jobs(first: 1, after: $after) {
            nodes { id state placement }
            pageInfo { endCursor hasNextPage }
        }
    }";
    let first = work.daemons.operate(page, json!({})).await;
    assert_eq!(
        first["data"]["jobs"]["nodes"][0]["id"],
        two.id.to_string(),
        "{first}"
    );
    assert_eq!(first["data"]["jobs"]["pageInfo"]["hasNextPage"], true);
    let after = &first["data"]["jobs"]["pageInfo"]["endCursor"];
    let next = work.daemons.operate(page, json!({"after": after})).await;
    assert_eq!(
        next["data"]["jobs"]["nodes"][0]["id"],
        one.id.to_string(),
        "{next}"
    );
    assert_eq!(next["data"]["jobs"]["pageInfo"]["hasNextPage"], false);

    let id = one.id.to_string();
    let stopped = work
        .daemons
        .operate("mutation($id: ID!) { stopJob(id: $id) }", json!({"id": id}))
        .await;
    assert_eq!(stopped["data"]["stopJob"], true, "{stopped}");
    let detail = work
        .daemons
        .operate(
            "query($id: ID!) { job(id: $id) { job { id state } log } }",
            json!({"id": id}),
        )
        .await;
    assert_eq!(
        detail["data"]["job"],
        json!({"job": {"id": id, "state": "STOPPED"}, "log": ""})
    );
}

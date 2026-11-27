use chrono::TimeDelta;
use serde_json::json;
use sqlx::PgPool;

use super::{PVE_01, Work};

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn container_action_api_requires_write_and_exposes_its_queued_state_to_readers(pool: PgPool) {
    let work = Work::new(&pool).await;
    let read = api_key(&work, "READ").await;
    let write = api_key(&work, "WRITE").await;
    let mutation = "mutation($input: ContainerActionInput!) { queueContainerAction(input:$input) { id state reason } }";
    let mut args =
        json!({"input":{"deviceId":PVE_01,"containerId":"a".repeat(64),"action":"RESTART"}});
    let denied = work
        .daemons
        .graphql(Some(&read), mutation, args.clone())
        .await;
    assert_eq!(denied["errors"][0]["extensions"]["code"], "FORBIDDEN");
    let short = args["input"]["containerId"].as_str().unwrap()[..12].to_owned();
    args["input"]["containerId"] = json!(short);
    let invalid = work
        .daemons
        .graphql(Some(&write), mutation, args.clone())
        .await;
    assert_eq!(invalid["errors"][0]["extensions"]["code"], "VALIDATION");
    let rows: i64 = sqlx::query_scalar("select count(*) from jobs.jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);

    args["input"]["containerId"] = json!("a".repeat(64));
    let response = work.daemons.graphql(Some(&write), mutation, args).await;
    let queued = &response["data"]["queueContainerAction"];
    assert_eq!(queued["state"], "queued", "{response}");
    let status = work
        .daemons
        .graphql(
            Some(&read),
            "query($id:ID!){ containerAction(id:$id){ id state reason } }",
            json!({"id":queued["id"]}),
        )
        .await;
    assert_eq!(&status["data"]["containerAction"], queued);
}

async fn api_key(work: &Work, scope: &str) -> String {
    let request = "mutation($input:ApiKeyInput!){ createApiKey(input:$input){ secret } }";
    let variables = json!({"input":{
        "name":format!("container-{}", scope.to_ascii_lowercase()), "scope":scope, "madeFor":"USER",
        "expiresAt": work.now() + TimeDelta::days(30)
    }});
    let granted = work.daemons.operate(request, variables).await;
    granted["data"]["createApiKey"]["secret"]
        .as_str()
        .unwrap()
        .to_owned()
}

//! The operator can submit files, and status exposes the result without them.
use super::{
    COMPOSE, ENVIRONMENT, PVE_01, PgPool, StatusCode, Work, final_report, hosting, json, preparing,
};

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn operator_apply_returns_status_without_either_input_file(pool: PgPool) {
    let work = Work::new(&pool).await;
    let query = "mutation($input: ApplicationApplyInput!) { queueApplicationApply(input:$input) { id applicationId deviceId state reason health } }";
    let input =
        json!({"input":{"deviceId":PVE_01,"composeFile":COMPOSE,"environmentFile":ENVIRONMENT}});
    let key = work
        .daemons
        .operate(
            "mutation($input:ApiKeyInput!) { createApiKey(input:$input) { secret } }",
            json!({"input":{"name":"apply-management","scope":"WRITE","madeFor":"USER",
            "expiresAt":work.now() + chrono::TimeDelta::days(14)}}),
        )
        .await;
    let bearer = key["data"]["createApiKey"]["secret"].as_str().unwrap();
    let forbidden = work
        .daemons
        .graphql(Some(bearer), query, input.clone())
        .await;
    assert_eq!(forbidden["errors"][0]["extensions"]["code"], "FORBIDDEN");
    assert!(!forbidden.to_string().contains("synthetic-apply-credential"));
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs.jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queued, 0);

    let daemon = work.daemons.token_for(PVE_01).await;
    let supported = hosting(PVE_01, &json!([]));
    assert_eq!(
        work.claim(&daemon, &supported).await.0,
        StatusCode::NO_CONTENT
    );
    let targets = work
        .daemons
        .operate("{ applicationApplyTargets { id } }", json!({}))
        .await;
    assert_eq!(
        targets["data"]["applicationApplyTargets"],
        json!([{"id":PVE_01}])
    );

    for cause in ["secret_failed", "migration_held"] {
        let reply = work.daemons.operate(query, input.clone()).await;
        let accepted = &reply["data"]["queueApplicationApply"];
        check_result(&work, (bearer, &daemon), (&supported, accepted), cause).await;
        assert!(!reply.to_string().contains("synthetic-apply-credential"));
    }
}

async fn check_result(
    work: &Work,
    (bearer, daemon): (&str, &str),
    (supported, accepted): (&serde_json::Value, &serde_json::Value),
    cause: &str,
) {
    assert_eq!(accepted["deviceId"], PVE_01, "{accepted}");
    assert_eq!(accepted["state"], "queued");
    assert!(accepted["applicationId"].as_str().is_some());

    let (_, item) = work.claim(daemon, supported).await;
    assert_eq!(item["id"], accepted["id"]);
    preparing(work, daemon, &item).await;
    let mut report = final_report(&item, "failed");
    report["result"]["cause"] = json!(cause);
    assert_eq!(work.report(daemon, &report).await.0, StatusCode::OK);

    let result = work
        .daemons
        .graphql(
            Some(bearer),
            "query($id:ID!) { applicationApply(id:$id) { id state reason applicationId } }",
            json!({"id":accepted["id"]}),
        )
        .await;
    let found = &result["data"]["applicationApply"];
    assert_eq!(found["applicationId"], accepted["applicationId"]);
    assert_eq!(found["state"], "failed");
    assert_eq!(found["reason"], cause);
    assert!(!result.to_string().contains("synthetic-apply-credential"));
}

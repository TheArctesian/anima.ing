use super::{
    github::{Repo, list},
    kit,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

const QUEUE: &str =
    "mutation($input:SourceBuildInput!) {queueSourceBuild(input:$input){id state attempt}}";

#[sqlx::test(migrations = "./migrations")]
async fn connected_public_project_submits_a_pinned_source_job(pool: PgPool) {
    let kit = kit(&pool).await;
    let account = kit.connect().await;
    let mut public = Repo::new(1, "me/source");
    public.private = false;
    let mut private = Repo::new(2, "me/private");
    private.private = true;
    list(&kit.stub, &[public, private]);
    kit.run(account).await;
    let public = kit.project("source").await;
    let private = kit.project("private").await;
    let commit = "ab".repeat(20);
    let input = |project: &Value, commit: &str| {
        json!({"input":{"repository":project["repositories"][0]["id"],"commit":commit,
            "target":"LINUX_AARCH64","strategy":"RAILPACK"}})
    };
    for refused in [
        kit.post(QUEUE, input(&private, &commit)).await,
        kit.post(QUEUE, input(&public, "main")).await,
    ] {
        assert!(refused["errors"].is_array(), "{refused}");
    }
    let mut submission = input(&public, &commit);
    submission["input"]["request"] = json!(Uuid::now_v7());
    let (submitted, recovered) = tokio::join!(
        kit.post(QUEUE, submission.clone()),
        kit.post(QUEUE, submission.clone()),
    );
    let job = &submitted["data"]["queueSourceBuild"];
    assert_eq!(job["state"], "queued", "{submitted}");
    assert_eq!(&recovered["data"]["queueSourceBuild"], job);
    submission["input"]["strategy"] = json!("DOCKERFILE");
    let conflict = kit.post(QUEUE, submission).await;
    assert_eq!(conflict["errors"][0]["extensions"]["code"], "CONFLICT");
    let rows: Vec<(String, String, Value, i64)> = sqlx::query_as(
        "SELECT project_id::text, owner || '/' || name, parameters, limit_memory_bytes FROM jobs.jobs",
    )
    .fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 1);
    let (project, repository, parameters, memory) = &rows[0];
    assert_eq!(json!(project), public["id"]);
    assert_eq!(repository, "me/source");
    assert_eq!(
        parameters,
        &json!({"recipe":"source_image_v1","commit":commit,"target":"linux-aarch64",
        "strategy":"railpack","trigger":"operator","origin":"other"})
    );
    assert_eq!(*memory, 2_147_483_648);
    let status = kit
        .post(
            "query($id: ID!){sourceBuild(id:$id){id state attempt}}",
            json!({"id":job["id"]}),
        )
        .await;
    assert_eq!(&status["data"]["sourceBuild"], job);
}
#[cfg(unix)]
mod checkout;

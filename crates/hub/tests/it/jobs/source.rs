use super::{Work, build};
use anima_hub::{
    fleet::events::{ClientAddr, Operator},
    work::jobs::{self, Error},
};
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn source_jobs_refuse_invalid_build_parameters_before_entering_the_queue(pool: PgPool) {
    let work = Work::new(&pool).await;
    let mut source = build();
    source.parameters.as_object_mut().unwrap().remove("version");
    source.parameters["recipe"] = json!("source_image_v1");
    source.parameters["strategy"] = json!("auto");
    for (field, value) in [
        ("command", json!("untrusted-command")),
        ("commit", json!("main")),
        ("strategy", json!("shell")),
        ("origin", json!("unknown")),
        ("trigger", json!(null)),
    ] {
        let mut invalid = source.clone();
        invalid.parameters[field] = value;
        let result = jobs::enqueue(
            work.queue(),
            invalid,
            (Operator::hub(), ClientAddr("127.0.0.1".parse().unwrap())),
            work.now(),
        )
        .await;
        assert!(
            matches!(result, Err(Error::Invalid(_))),
            "{field}: {result:?}"
        );
    }
    let count: i64 = sqlx::query_scalar("select count(*) from jobs.jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "refused input must not leave queued work");
    let queued = work.enqueue(source.clone()).await;
    assert_eq!(
        work.job(queued.id).await.0.asked.parameters,
        source.parameters
    );
}

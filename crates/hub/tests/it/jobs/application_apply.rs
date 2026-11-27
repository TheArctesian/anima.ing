//! The real K10 routes enforce the application binding and the one-use file lifetime.
mod graphql;

use anima_hub::{
    fleet::{
        credentials::Secret,
        events::{Actor, ActorKind, ClientAddr, Operator, TokenKind},
    },
    ids::{DeviceId, JobId},
    work::jobs::{self, ApplicationFiles, Job},
};
use axum::http::StatusCode;
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::{Work, report_of};
use crate::{
    daemon::harness::{DOCKER_HOST, PERMITTED, PVE_01},
    hub::test_vault,
};

const APPLICATION: &str = "0190a000-0000-7000-8002-000000000003";
const COMPOSE: &str =
    "services:\n  web:\n    image: example/web:1\n    environment:\n      VALUE: ${VALUE}\n";
const ENVIRONMENT: &str = "VALUE=synthetic-apply-credential\n";

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn apply_files_are_sealed_and_released_once_to_the_current_preparing_holder(pool: PgPool) {
    let work = Work::new(&pool).await;
    let daemon = work.daemons.token_for(PVE_01).await;
    let other = work.daemons.token_for(DOCKER_HOST).await;
    let job = enqueue(&work, PVE_01, (true, Actor::for_tests()))
        .await
        .unwrap();
    let stored: Vec<Vec<u8>> =
        sqlx::query_scalar("SELECT sealed FROM jobs.application_files WHERE job_id = $1")
            .bind(job.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(stored.len(), 2);
    for bytes in &stored {
        assert!(!String::from_utf8_lossy(bytes).contains("synthetic-apply-credential"));
        assert!(!String::from_utf8_lossy(bytes).contains("example/web:1"));
    }

    let mut old = hosting(PVE_01, &json!([]));
    old["types"] = json!(["container_action"]);
    assert_eq!(work.claim(&daemon, &old).await.0, StatusCode::NO_CONTENT);
    assert_eq!(
        work.claim(&other, &hosting(DOCKER_HOST, &json!([])))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (status, item) = work.claim(&daemon, &hosting(PVE_01, &json!([]))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(item["id"], job.id.to_string());
    assert_eq!(item["type"], "application_apply");
    assert_eq!(item["secrets"], json!(["compose_file", "environment_file"]));
    assert!(!item.to_string().contains("synthetic-apply-credential"));
    assert_eq!(item["limits"]["memoryBytes"], 0);

    let compose = fetch_for(&item, "compose_file");
    deny(&work, &daemon, &compose).await;
    preparing(&work, &daemon, &item).await;
    deny(&work, &other, &compose).await;
    let mut wrong = compose.clone();
    wrong["attempt"] = json!(2);
    deny(&work, &daemon, &wrong).await;
    released(&work, &daemon, &compose, COMPOSE).await;
    deny(&work, &daemon, &compose).await;

    let environment = fetch_for(&item, "environment_file");
    work.daemons.clock.advance(TimeDelta::seconds(91));
    deny(&work, &daemon, &environment).await;
    preparing(&work, &daemon, &item).await;
    released(&work, &daemon, &environment, ENVIRONMENT).await;
    deny(&work, &daemon, &environment).await;
    assert_fetch_audit(&pool, job.id, &["compose_file", "environment_file"]).await;
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn a_finishing_apply_excludes_another_on_its_application_and_stop_erases_unsent_files(
    pool: PgPool,
) {
    let work = Work::new(&pool).await;
    let api_key = Actor::request(ActorKind::User, TokenKind::ApiKey, Some("write".to_owned()));
    assert!(enqueue(&work, PVE_01, (true, api_key)).await.is_err());
    let daemon = work.daemons.token_for(PVE_01).await;
    let first = enqueue(&work, PVE_01, (true, Actor::for_tests()))
        .await
        .unwrap();
    assert!(
        enqueue(&work, DOCKER_HOST, (true, Actor::for_tests()))
            .await
            .is_err()
    );
    work.daemons.clock.advance(TimeDelta::seconds(1));
    let other = enqueue(&work, PVE_01, (false, Actor::for_tests()))
        .await
        .unwrap();
    let (_, item) = work.claim(&daemon, &hosting(PVE_01, &json!([]))).await;
    assert_eq!(item["id"], first.id.to_string());
    preparing(&work, &daemon, &item).await;
    released(&work, &daemon, &fetch_for(&item, "compose_file"), COMPOSE).await;
    released(
        &work,
        &daemon,
        &fetch_for(&item, "environment_file"),
        ENVIRONMENT,
    )
    .await;
    for state in ["running", "finishing"] {
        assert_eq!(
            work.report(&daemon, &report_of(&item, state)).await.0,
            StatusCode::OK
        );
    }
    let holds = json!([{"id":item["id"],"attempt":1}]);
    assert_eq!(
        work.claim(&daemon, &hosting(PVE_01, &holds)).await.0,
        StatusCode::NO_CONTENT
    );
    let mut done = final_report(&item, "done");
    assert_eq!(work.report(&daemon, &done).await.0, StatusCode::BAD_REQUEST);
    done["result"]["health"] = json!("healthy");
    assert_eq!(work.report(&daemon, &done).await.0, StatusCode::OK);
    assert_eq!(
        work.job(first.id).await.0.result.unwrap().health,
        Some(daemon_core::wire::ApplicationHealth::Healthy)
    );

    let (_, item) = work.claim(&daemon, &hosting(PVE_01, &json!([]))).await;
    assert_eq!(item["id"], other.id.to_string());
    preparing(&work, &daemon, &item).await;
    jobs::stop(
        work.queue(),
        (other.id, daemon_core::wire::StopReason::Operator),
        operator(Actor::for_tests()),
        work.now(),
    )
    .await
    .unwrap()
    .unwrap();
    deny(&work, &daemon, &fetch_for(&item, "compose_file")).await;
    assert_eq!(
        work.report(&daemon, &final_report(&item, "stopped"))
            .await
            .0,
        StatusCode::OK
    );
    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM jobs.application_files WHERE job_id = $1")
            .bind(other.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0);
    assert_fetch_audit(&pool, other.id, &[]).await;
}

async fn enqueue(
    work: &Work,
    device: &str,
    (pull, actor): (bool, Actor),
) -> Result<Job, jobs::Error> {
    jobs::enqueue_application_apply(
        (work.queue(), &test_vault()),
        (
            APPLICATION.parse::<Uuid>().unwrap(),
            device.parse::<DeviceId>().unwrap(),
            pull,
            true,
        ),
        ApplicationFiles {
            compose: Secret::new(COMPOSE.as_bytes().to_vec()),
            environment: Some(Secret::new(ENVIRONMENT.as_bytes().to_vec())),
        },
        (operator(actor), work.now()),
    )
    .await
}

fn operator(actor: Actor) -> jobs::Asker {
    (
        Operator {
            actor,
            ..Operator::hub()
        },
        ClientAddr(PERMITTED.parse().unwrap()),
    )
}

fn hosting(device: &str, holds: &Value) -> Value {
    json!({
        "device": device, "switch":"off", "applications":"on",
        "types":["container_action","application_apply"], "kinds":["host"], "os":"linux",
        "architecture":"x86_64", "free":{"slots":0,"memoryBytes":0,"diskBytes":0}, "holds": holds,
        "unclean":false
    })
}

fn fetch_for(item: &Value, purpose: &str) -> Value {
    json!({"id":item["id"],"attempt":item["attempt"],"purpose":purpose})
}

async fn deny(work: &Work, token: &str, input: &Value) {
    let (status, body) = work.post("secret", Some(token), input).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body, json!({"error":"SECRET_REFUSED"}));
}

async fn released(work: &Work, token: &str, input: &Value, expected: &str) {
    let (status, body) = work.post("secret", Some(token), input).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["value"], expected);
    assert!(body["expiresAt"].is_string());
}

async fn preparing(work: &Work, token: &str, item: &Value) {
    assert_eq!(
        work.report(token, &report_of(item, "preparing")).await.0,
        StatusCode::OK
    );
}

fn final_report(item: &Value, state: &str) -> Value {
    let outcome = if state == "done" { "succeeded" } else { state };
    json!({"id":item["id"],"attempt":item["attempt"],"state":state,
        "result":{"outcome":outcome,"cause":null,"seconds":1,
            "cleanup":{"isolation":"none","checkout":"none","secrets":"none","keychain":"none"}}})
}

async fn assert_fetch_audit(pool: &PgPool, id: JobId, purposes: &[&str]) {
    let entries: Vec<Value> = sqlx::query_scalar(
        "SELECT change FROM events.audit_log WHERE target_type = 'job'
         AND target_id = $1 AND action = 'fetch' ORDER BY id",
    )
    .bind(id.to_string())
    .fetch_all(pool)
    .await
    .unwrap();
    let expected: Vec<_> = purposes.iter().map(|purpose| {
        json!({"purpose":purpose,"attempt":1,"application":APPLICATION,"device":PVE_01})
    }).collect();
    assert_eq!(entries, expected);
}

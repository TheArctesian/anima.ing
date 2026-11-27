mod graphql;

use anima_hub::{
    fleet::events::{Actor, ActorKind, ClientAddr, Operator, TokenKind},
    ids::DeviceId,
    work::jobs::{self, Job, State},
};
use axum::http::StatusCode;
use chrono::TimeDelta;
use daemon_core::wire::{ContainerAction, ContainerVerb};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{Work, build, claim_of, report_of};
use crate::daemon::harness::{DOCKER_HOST, PERMITTED, PVE_01};

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn a_named_host_can_claim_a_stop_while_it_holds_a_build_and_another_mutation(pool: PgPool) {
    let work = Work::new(&pool).await;
    let first = work.daemons.token_for(PVE_01).await;
    let other = work.daemons.token_for(DOCKER_HOST).await;
    let built = work.enqueue(build()).await;
    let (_, job) = work.claim(&first, &claim_of(PVE_01, &json!([]))).await;
    assert_eq!(job["id"], built.id.to_string());

    let restart = enqueue(&work, &action(ContainerVerb::Restart, 'a')).await;
    let stop = enqueue(&work, &action(ContainerVerb::Stop, 'b')).await;
    let holds = json!([held(&job)]);
    assert_eq!(
        work.claim(&other, &hosting(DOCKER_HOST, &json!([])))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        work.claim(&first, &claim_of(PVE_01, &holds)).await.0,
        StatusCode::NO_CONTENT
    );
    let (status, item) = work.claim(&first, &hosting(PVE_01, &holds)).await;
    assert_eq!(status, StatusCode::OK);
    assert_item(&item, &stop);
    finish(&work, &first, &item).await;

    let (_, item) = work.claim(&first, &hosting(PVE_01, &holds)).await;
    assert_item(&item, &restart);
    let waiting = enqueue(&work, &action(ContainerVerb::Start, 'c')).await;
    let holds = json!([held(&job), held(&item)]);
    assert_eq!(
        work.claim(&first, &hosting(PVE_01, &holds)).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(work.job(waiting.id).await.0.state, State::Queued);
    let another_stop = enqueue(&work, &action(ContainerVerb::Stop, 'd')).await;
    let (_, item) = work.claim(&first, &hosting(PVE_01, &holds)).await;
    assert_item(&item, &another_stop);
    assert_eq!(work.job(built.id).await.0.state, State::Claimed);
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn a_host_attempt_keeps_its_place_during_a_lease_gap_and_never_requeues(pool: PgPool) {
    let work = Work::new(&pool).await;
    let token = work.daemons.token_for(PVE_01).await;
    let lost = enqueue(&work, &action(ContainerVerb::Restart, 'a')).await;
    let (_, item) = work.claim(&token, &hosting(PVE_01, &json!([]))).await;
    work.daemons.clock.advance(TimeDelta::seconds(91));
    assert_eq!(jobs::end_lost(work.queue(), work.now()).await.unwrap(), 0);
    assert_eq!(work.job(lost.id).await.0.state, State::Claimed);
    assert_eq!(
        work.report(&token, &report_of(&item, "preparing")).await.0,
        StatusCode::OK
    );
    work.daemons.clock.advance(TimeDelta::seconds(30));
    let (status, ended) = work.report(&token, &report_of(&item, "running")).await;
    assert_eq!(
        (status, &ended["stop"]["reason"]),
        (StatusCode::OK, &json!("expired"))
    );
    work.daemons.clock.advance(TimeDelta::seconds(299));
    assert_eq!(jobs::end_lost(work.queue(), work.now()).await.unwrap(), 1);
    assert_orphan(&work, &lost, 1).await;
    assert_eq!(
        work.report(&token, &report_of(&item, "running")).await.0,
        StatusCode::CONFLICT
    );
    let unclaimed = enqueue(&work, &action(ContainerVerb::Stop, 'a')).await;
    work.daemons.clock.advance(TimeDelta::minutes(10));
    assert_eq!(jobs::end_lost(work.queue(), work.now()).await.unwrap(), 1);
    assert_orphan(&work, &unclaimed, 0).await;
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn a_reconnecting_host_can_close_its_same_running_attempt_as_indeterminate(pool: PgPool) {
    let work = Work::new(&pool).await;
    let token = work.daemons.token_for(PVE_01).await;
    let restart = enqueue(&work, &action(ContainerVerb::Restart, 'a')).await;
    let (status, item) = work.claim(&token, &hosting(PVE_01, &json!([]))).await;
    assert_eq!(status, StatusCode::OK);
    assert_item(&item, &restart);
    for phase in ["preparing", "running"] {
        assert_eq!(
            work.report(&token, &report_of(&item, phase)).await.0,
            StatusCode::OK
        );
    }

    work.daemons.clock.advance(TimeDelta::seconds(91));
    let recovered = hosting(PVE_01, &json!([held(&item)]));
    assert_eq!(
        work.claim(&token, &recovered).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(work.job(restart.id).await.0.state, State::Running);
    let mut report = host_result(&item, "failed", "failed");
    report["result"]["cause"] = json!("orphan");
    let (status, first) = work.report(&token, &report).await;
    assert_eq!(status, StatusCode::OK);
    assert_orphan(&work, &restart, 1).await;

    work.daemons.clock.advance(TimeDelta::seconds(1));
    assert_eq!(work.report(&token, &report).await, (StatusCode::OK, first));
    assert_eq!(
        work.claim(&token, &hosting(PVE_01, &json!([]))).await.0,
        StatusCode::NO_CONTENT
    );
    let ended: i64 = sqlx::query_scalar(
        "select count(*) from events.audit_log where target_type = 'job' and target_id = $1 and action = 'end'",
    )
    .bind(restart.id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ended, 1);
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn host_refusal_is_final_and_does_not_penalize_job_capacity(pool: PgPool) {
    let work = Work::new(&pool).await;
    let token = work.daemons.token_for(PVE_01).await;
    let key = Actor::request(
        ActorKind::User,
        TokenKind::ApiKey,
        Some("fixture-key".into()),
    );
    let api_action = submit(&work, &action(ContainerVerb::Restart, 'a'), key.clone())
        .await
        .unwrap();
    let login_action = enqueue(&work, &action(ContainerVerb::Start, 'b')).await;
    assert!(
        submit(&work, &action(ContainerVerb::Restart, 'a'), key.clone())
            .await
            .is_err()
    );
    assert!(
        submit(&work, &action(ContainerVerb::Stop, 'a'), Actor::hub())
            .await
            .is_err()
    );
    let (_, item) = work.claim(&token, &hosting(PVE_01, &json!([]))).await;
    assert_item(&item, &login_action);
    let mut result = host_result(&item, "refused", "refused");
    result["result"]["cause"] = json!("protected");
    assert_eq!(work.report(&token, &result).await.0, StatusCode::OK);
    assert_eq!(work.report(&token, &result).await.0, StatusCode::OK);
    let failed = work.job(login_action.id).await.0;
    assert_eq!(failed.state, State::Failed);
    assert_eq!(failed.reason.as_deref(), Some("protected"));
    assert_eq!(work.job(api_action.id).await.0.state, State::Queued);
    let faults: i64 = sqlx::query_scalar("select count(*) from jobs.faults")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(faults, 0);
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn one_api_key_cannot_queue_more_than_six_actions_a_minute_or_twelve_open(pool: PgPool) {
    let work = Work::new(&pool).await;
    let key = Actor::request(
        ActorKind::Agent,
        TokenKind::ApiKey,
        Some("fixture-key".into()),
    );
    for digits in ["012345", "6789ab"] {
        for digit in digits.chars() {
            submit(&work, &action(ContainerVerb::Start, digit), key.clone())
                .await
                .unwrap();
        }
        assert!(
            submit(&work, &action(ContainerVerb::Start, 'c'), key.clone())
                .await
                .is_err()
        );
        work.daemons.clock.advance(TimeDelta::seconds(61));
    }
    assert!(
        submit(&work, &action(ContainerVerb::Start, 'c'), key)
            .await
            .is_err()
    );
    let other_key = Actor::request(
        ActorKind::Agent,
        TokenKind::ApiKey,
        Some("other-key".into()),
    );
    assert!(
        submit(&work, &action(ContainerVerb::Start, 'c'), other_key)
            .await
            .is_ok()
    );
}

fn action(action: ContainerVerb, digit: char) -> ContainerAction {
    ContainerAction {
        container: digit.to_string().repeat(64),
        action,
    }
}

async fn enqueue(work: &Work, item: &ContainerAction) -> Job {
    submit(work, item, Actor::for_tests()).await.unwrap()
}

async fn submit(work: &Work, item: &ContainerAction, actor: Actor) -> Result<Job, jobs::Error> {
    let caller = Operator {
        actor,
        ..Operator::hub()
    };
    let asker = (caller, ClientAddr(PERMITTED.parse().unwrap()));
    let device = PVE_01.parse::<DeviceId>().unwrap();
    jobs::enqueue_container_action(work.queue(), (device, item), asker, work.now()).await
}

fn hosting(device: &str, holds: &Value) -> Value {
    json!({
        "device": device, "switch":"off", "applications":"on",
        "types":["container_action"], "kinds":["host"], "os":"linux", "architecture":"x86_64",
        "free":{"slots":0,"memoryBytes":0,"diskBytes":0}, "holds": holds, "unclean":true,
    })
}

fn held(item: &Value) -> Value {
    json!({"id":item["id"],"attempt":item["attempt"]})
}

fn assert_item(item: &Value, queued: &Job) {
    assert_eq!(item["id"], queued.id.to_string());
    assert_eq!(item["type"], "container_action");
    assert_eq!(item["kind"], "host");
    assert_eq!(item["attempt"], 1);
    assert_eq!(item["secrets"], json!([]));
    assert_eq!(item["project"], Value::Null);
    assert!(item.get("repository").is_none());
}

async fn finish(work: &Work, token: &str, item: &Value) {
    for phase in ["preparing", "running", "finishing"] {
        assert_eq!(
            work.report(token, &report_of(item, phase)).await.0,
            StatusCode::OK
        );
    }
    let done = host_result(item, "done", "succeeded");
    assert_eq!(work.report(token, &done).await.0, StatusCode::OK);
    assert_eq!(work.report(token, &done).await.0, StatusCode::OK);
}

fn host_result(item: &Value, state: &str, outcome: &str) -> Value {
    json!({"id":item["id"],"attempt":item["attempt"],"state":state,
        "result":{"outcome":outcome,"cause":null,"seconds":1,
        "cleanup":{"isolation":"none","checkout":"none","secrets":"none","keychain":"none"}}})
}

async fn assert_orphan(work: &Work, item: &Job, attempts: u32) {
    let final_job = work.job(item.id).await.0;
    assert_eq!(
        (final_job.state, final_job.attempt),
        (State::Failed, attempts)
    );
    assert_eq!(final_job.reason.as_deref(), Some("orphan"));
}

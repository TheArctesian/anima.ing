use anima_hub::{
    fleet::{
        compute::policy::{self, DevicePolicy},
        events::{Actor, ClientAddr, Operator},
    },
    work::jobs::{self, State},
};
use axum::http::StatusCode;
use chrono::TimeDelta;
use daemon_core::wire::{ContainerAction, ContainerVerb, MODEL_TYPES, WorkType};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::{Work, build, claim_of, report_of};
use crate::daemon::harness::{DOCKER_HOST, PERMITTED, PVE_01};

fn mixed(holds: &[Value]) -> Value {
    let mut claim = claim_of(PVE_01, &json!(holds));
    claim["threads"] = json!("on");
    claim["kinds"] = json!(["container", "model", "person"]);
    let mut types = vec![json!("build"), json!("runner"), json!("agent_thread")];
    types.extend(MODEL_TYPES.iter().map(|kind| json!(kind)));
    claim["types"] = json!(types);
    claim
}

fn held(answer: &Value) -> Value {
    json!({"id": answer["id"], "attempt": answer["attempt"]})
}

async fn queue(
    work: &Work,
    (kind, target): (WorkType, Option<&str>),
    (memory, disk): (i64, i64),
    age: i64,
) -> Uuid {
    let id = Uuid::now_v7();
    let parameters = match kind {
        WorkType::ModelDrain => json!({"seconds": 60}),
        WorkType::ServerStop => json!({"server": Uuid::now_v7()}),
        WorkType::ModelUnload => json!({"server": Uuid::now_v7(), "copy": Uuid::now_v7()}),
        WorkType::ModelMeasure => {
            json!({"server": Uuid::now_v7(), "copy": Uuid::now_v7(), "probe": "text_short"})
        }
        WorkType::RuntimeInstall => {
            json!({"runtime": "llama_server", "version": "b8100", "backend": "cpu"})
        }
        WorkType::CopyDelete => json!({"copy": Uuid::now_v7()}),
        _ => json!({}),
    };
    sqlx::query(
        "INSERT INTO jobs.jobs (id, work_type, kind, actor, parameters, machine_id,
             state, created_at, state_changed_at, limit_seconds, limit_idle_seconds,
             limit_memory_bytes, limit_disk_bytes, limit_orphan_seconds)
         VALUES ($1, $2, 'model', 'Agent', $3, $4::uuid, 'queued', $5, $5, 120, 30, $6, $7, 120)",
    )
    .bind(id)
    .bind(kind.as_str())
    .bind(parameters)
    .bind(target)
    .bind(work.now() - TimeDelta::seconds(age))
    .bind(memory)
    .bind(disk)
    .execute(&work.pool)
    .await
    .unwrap();
    id
}

async fn hardware(work: &Work, power: &str) {
    sqlx::query(
        "INSERT INTO metrics.hardware (device_id, system_memory_bytes, free_disk_bytes, cpu_features,
            backend_names, backend_reasons, power_source, found_at)
         VALUES ($1::uuid, 1000, 1000, '{}', '{}', '{}', $2, $3)
         ON CONFLICT (device_id) DO UPDATE SET power_source = EXCLUDED.power_source",
    )
    .bind(PVE_01)
    .bind(power)
    .bind(work.now())
    .execute(&work.pool)
    .await
    .unwrap();
}

async fn take(work: &Work, token: &str, claim: &Value, id: Uuid) -> Value {
    let (status, answer) = work.claim(token, claim).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["id"], id.to_string(), "{answer}");
    answer
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn mixed_local_work_off_allows_only_advertised_reductions_on_the_right_device(pool: PgPool) {
    let work = Work::new(&pool).await;
    let token = work.daemons.token_for(PVE_01).await;
    let other_token = work.daemons.token_for(DOCKER_HOST).await;
    queue(
        &work,
        (WorkType::ModelDrain, Some(DOCKER_HOST)),
        (0, 0),
        120,
    )
    .await;
    queue(&work, (WorkType::RuntimeInstall, None), (0, 0), 100).await;
    let delete = queue(&work, (WorkType::CopyDelete, Some(PVE_01)), (0, 0), 10).await;
    let mut claim = mixed(&[]);
    claim["switch"] = json!("off");
    claim["applications"] = json!("off");
    claim["free"]["slots"] = json!(0);
    claim["threads"] = json!("off");
    assert_eq!(work.claim(&token, &claim).await.0, StatusCode::NO_CONTENT);

    claim["threads"] = json!("on");
    claim["types"] = json!(["agent_thread", "runtime_install"]);
    assert_eq!(work.claim(&token, &claim).await.0, StatusCode::NO_CONTENT);
    claim["types"] = json!(["model_drain", "copy_delete", "runtime_install"]);
    let answer = take(&work, &token, &claim, delete).await;
    assert_eq!(answer["kind"], "model");
    assert_eq!(answer["type"], "copy_delete");
    assert_eq!(answer["actor"], "Agent");
    assert_eq!(answer["project"], Value::Null);
    assert_eq!(answer["attempt"], 1);
    assert_eq!(answer.as_object().unwrap().len(), 10);
    assert!(answer.get("repository").is_none());
    let report = report_of(&answer, "preparing");
    assert_eq!(
        work.report(&other_token, &report).await.0,
        StatusCode::CONFLICT
    );
    let mut conn = pool.acquire().await.unwrap();
    jobs::set_device_work(&mut conn, PVE_01.parse().unwrap(), false, work.now())
        .await
        .unwrap();
    assert_eq!(work.report(&token, &report).await.0, StatusCode::FORBIDDEN);
    jobs::set_device_work(&mut conn, PVE_01.parse().unwrap(), true, work.now())
        .await
        .unwrap();
    assert_eq!(work.report(&token, &report).await.0, StatusCode::OK);
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn device_policy_checks_power_and_shares_but_keeps_reducing_work_available(pool: PgPool) {
    let work = Work::new(&pool).await;
    let token = work.daemons.token_for(PVE_01).await;
    let device = PVE_01.parse().unwrap();
    let policy = DevicePolicy {
        memory_share: 50,
        disk_share: 25,
        ..DevicePolicy::default()
    };
    let mut tx = pool.begin().await.unwrap();
    policy::set_policy(&mut tx, device, &policy, work.now())
        .await
        .unwrap();
    tx.commit().await.unwrap();

    queue(&work, (WorkType::ModelMeasure, Some(PVE_01)), (501, 1), 120).await;
    queue(&work, (WorkType::ModelMeasure, Some(PVE_01)), (1, 251), 110).await;
    let ordinary = queue(
        &work,
        (WorkType::ModelMeasure, Some(PVE_01)),
        (500, 250),
        100,
    )
    .await;
    let claim = mixed(&[]);
    assert_eq!(work.claim(&token, &claim).await.0, StatusCode::NO_CONTENT);

    hardware(&work, "battery").await;
    let report = queue(&work, (WorkType::ModelReport, None), (900, 900), 10).await;
    let reducing = take(&work, &token, &claim, report).await;
    let claim = mixed(&[held(&reducing)]);
    assert_eq!(work.claim(&token, &claim).await.0, StatusCode::NO_CONTENT);

    hardware(&work, "mains").await;
    let mut claim = claim;
    claim["kinds"] = json!(["model"]);
    claim["types"] = json!(["model_measure"]);
    claim["free"] = json!({"slots": 0, "memoryBytes": 0, "diskBytes": 0});
    take(&work, &token, &claim, ordinary).await;
}

async fn host(work: &Work, verb: ContainerVerb, digit: char) -> Uuid {
    let operator = Operator {
        actor: Actor::for_tests(),
        ..Operator::hub()
    };
    let action = ContainerAction {
        action: verb,
        container: digit.to_string().repeat(64),
    };
    let caller = (operator, ClientAddr(PERMITTED.parse().unwrap()));
    jobs::enqueue_container_action(
        work.queue(),
        (PVE_01.parse().unwrap(), &action),
        caller,
        work.now(),
    )
    .await
    .unwrap()
    .id
    .into()
}

fn host_offer(mut claim: Value) -> Value {
    claim["applications"] = json!("on");
    claim["kinds"].as_array_mut().unwrap().push(json!("host"));
    claim["types"]
        .as_array_mut()
        .unwrap()
        .push(json!("container_action"));
    claim
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn container_priority_and_model_leases_are_independent_of_job_slots(pool: PgPool) {
    let work = Work::new(&pool).await;
    let token = work.daemons.token_for(PVE_01).await;
    let build = work.enqueue(build()).await;
    let regular = take(
        &work,
        &token,
        &claim_of(PVE_01, &json!([])),
        build.id.into(),
    )
    .await;
    let restart = host(&work, ContainerVerb::Restart, 'a').await;
    let stop = host(&work, ContainerVerb::Stop, 'b').await;
    let remove = queue(&work, (WorkType::CopyDelete, Some(PVE_01)), (0, 0), 100).await;
    let unload = queue(&work, (WorkType::ModelUnload, Some(PVE_01)), (0, 0), 90).await;
    let server_stop = queue(&work, (WorkType::ServerStop, Some(PVE_01)), (0, 0), 80).await;
    let drain = queue(&work, (WorkType::ModelDrain, Some(PVE_01)), (0, 0), 70).await;
    let measure = queue(&work, (WorkType::ModelMeasure, Some(PVE_01)), (0, 0), 60).await;
    queue(&work, (WorkType::ModelMeasure, Some(PVE_01)), (0, 0), 50).await;

    let mut holds = vec![held(&regular)];
    for id in [stop, restart, drain, server_stop, measure] {
        let mut claim = host_offer(mixed(&holds));
        claim["free"]["slots"] = json!(0);
        let answer = take(&work, &token, &claim, id).await;
        holds.push(held(&answer));
    }
    assert_eq!(
        work.claim(&token, &host_offer(mixed(&holds))).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(work.job(build.id).await.0.state, State::Claimed);
    for id in [remove, unload] {
        assert_eq!(work.job(id.into()).await.0.state, State::Queued);
    }
    let stopped = holds.remove(4);
    for phase in ["preparing", "running", "finishing", "done"] {
        assert_eq!(
            work.report(&token, &report_of(&stopped, phase)).await.0,
            StatusCode::OK
        );
    }
    take(&work, &token, &host_offer(mixed(&holds)), unload).await;
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn manual_and_cooldown_work_off_block_new_model_claims(pool: PgPool) {
    let work = Work::new(&pool).await;
    let token = work.daemons.token_for(PVE_01).await;
    let claim = mixed(&[]);
    let device = PVE_01.parse().unwrap();
    let mut conn = pool.acquire().await.unwrap();
    jobs::set_device_work(&mut conn, device, false, work.now())
        .await
        .unwrap();
    let drain = queue(&work, (WorkType::ModelDrain, Some(PVE_01)), (0, 0), 10).await;
    assert_eq!(work.claim(&token, &claim).await.0, StatusCode::NO_CONTENT);
    jobs::set_device_work(&mut conn, device, true, work.now())
        .await
        .unwrap();
    sqlx::query("UPDATE jobs.machines SET off_until = $2 WHERE device_id = $1")
        .bind(device)
        .bind(work.now() + TimeDelta::seconds(5))
        .execute(&mut *conn)
        .await
        .unwrap();
    assert_eq!(work.claim(&token, &claim).await.0, StatusCode::NO_CONTENT);
    work.daemons.clock.advance(TimeDelta::seconds(5));
    take(&work, &token, &claim, drain).await;
}

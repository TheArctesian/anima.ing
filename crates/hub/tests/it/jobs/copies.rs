//! The pull and the removal of a copy through the hub's routes (task L1-7; K11 Sections 2, 3,
//! 7 and 11; PULL-001, PULL-007, PULL-009): `pullModel` puts a `model_pull` for the named
//! machine in the queue, the daemon of that machine claims and reports it, and `modelCopies`
//! shows its state and progress; `removeModelCopy` asks for a `copy_delete`.

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::{Work, model::model_claim};
use crate::daemon::harness::PVE_01;

const TAG: &str = "8b-q4_k_m";
const FILE: &str = "Qwen3-8B-Q4_K_M.gguf";

const COPIES: &str = "{ modelCopies { id address state reason fileCount sizeBytes
    place { kind id name } progress { bytes totalBytes bytesPerSecond secondsLeft } } }";
const PULL: &str = "mutation($deviceId: ID!, $model: String!) {
    pullModel(deviceId: $deviceId, model: $model) { id address state place { kind id } } }";
const REMOVE: &str = "mutation($id: ID!) { removeModelCopy(id: $id) { id state } }";

/// One model of the library, `qwen3`, with one variant of one file of 100 bytes.
async fn seed_library(pool: &sqlx::PgPool) {
    let model: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO compute.models (id, name, added_at) VALUES (gen_random_uuid(), 'qwen3', now()) \
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO compute.variants (model_id, tag, format, quantisation, repository, revision) \
         VALUES ($1, $2, 'gguf', 'Q4_K_M', 'unsloth/Qwen3-8B-GGUF', $3)",
    )
    .bind(model)
    .bind(TAG)
    .bind("a".repeat(40))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO compute.variant_files (model_id, tag, path, size_bytes, sha256) \
         VALUES ($1, $2, $3, 100, $4)",
    )
    .bind(model)
    .bind(TAG)
    .bind(FILE)
    .bind("b".repeat(64))
    .execute(pool)
    .await
    .unwrap();
}

async fn copies(work: &Work) -> Vec<Value> {
    let answer = work.daemons.operate(COPIES, json!({})).await;
    answer["data"]["modelCopies"]
        .as_array()
        .unwrap_or_else(|| panic!("{answer}"))
        .clone()
}

async fn pull(work: &Work, model: &str) -> Value {
    let variables = json!({ "deviceId": PVE_01, "model": model });
    work.daemons.operate(PULL, variables).await
}

fn report(item: &Value, state: &str, extra: Value) -> Value {
    let mut body = json!({ "id": item["id"], "attempt": item["attempt"], "state": state });
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        body.extend(extra.clone());
    }
    body
}

fn done(copy: &Value, copy_state: &str) -> Value {
    json!({ "result": { "outcome": "succeeded", "cause": null, "seconds": 9,
        "cleanup": { "isolation": "none", "checkout": "none", "secrets": "none", "keychain": "none" },
        "model": { "copy": copy, "copyState": copy_state, "bytes": 100, "files": 1 } } })
}

/// Claims the next item and reports it through `states`; the last report has `last`.
async fn run(work: &Work, token: &str, states: &[&str], last: Value) -> Value {
    let (status, item) = work.claim(token, &model_claim(&json!([]))).await;
    assert_eq!(status, StatusCode::OK, "{item}");
    for (step, state) in states.iter().enumerate() {
        let extra = if step + 1 == states.len() {
            last.clone()
        } else {
            json!({})
        };
        let (status, body) = work.report(token, &report(&item, state, extra)).await;
        assert_eq!(status, StatusCode::OK, "{state}: {body}");
    }
    item
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn pull_001_a_pull_goes_to_the_named_machine_and_shows_its_progress(pool: sqlx::PgPool) {
    seed_library(&pool).await;
    let work = Work::new(&pool).await;
    let token = work.daemons.token_for(PVE_01).await;
    let (status, _) = work.claim(&token, &model_claim(&json!([]))).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "the hub learns the machine");

    let bare = pull(&work, "qwen3").await;
    assert_eq!(
        bare["errors"][0]["extensions"]["code"], "VALIDATION",
        "{bare}"
    );
    assert!(
        bare.to_string().contains("qwen3:8b-q4_k_m"),
        "the tags are named: {bare}"
    );

    let asked = pull(&work, &format!("qwen3:{TAG}")).await;
    let copy = &asked["data"]["pullModel"];
    assert_eq!(copy["state"], "QUEUED", "{asked}");
    assert_eq!(copy["place"], json!({ "kind": "MACHINE", "id": PVE_01 }));

    let (status, item) = work.claim(&token, &model_claim(&json!([]))).await;
    assert_eq!(status, StatusCode::OK, "{item}");
    assert_eq!(item["type"], "model_pull");
    assert_eq!(item["actor"], "User");
    let parameters = &item["parameters"];
    assert_eq!(parameters["copy"], copy["id"]);
    assert_eq!(parameters["source"]["organisation"], "unsloth");
    assert_eq!(parameters["files"][0]["name"], FILE);
    assert_eq!(parameters["files"][0]["sizeBytes"], 100);
    assert_eq!(item["secrets"], json!([]));

    for state in ["preparing", "running"] {
        let (status, body) = work.report(&token, &report(&item, state, json!({}))).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let progress = json!({ "progress": { "bytes": 40, "totalBytes": 100, "bytesPerSecond": 10 } });
    let (status, body) = work
        .report(&token, &report(&item, "running", progress))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed = copies(&work).await;
    assert_eq!(listed[0]["state"], "DOWNLOADING");
    assert_eq!(listed[0]["progress"]["secondsLeft"], 6.0);
    assert_eq!(listed[0]["place"]["name"], "pve-01");

    let finish = done(&copy["id"], "downloaded");
    for (state, extra) in [("finishing", json!({})), ("done", finish)] {
        let (status, body) = work.report(&token, &report(&item, state, extra)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let listed = copies(&work).await;
    assert_eq!(listed[0]["state"], "DOWNLOADED");
    assert_eq!(listed[0]["progress"], Value::Null);

    let again = pull(&work, &format!("qwen3:{TAG}")).await;
    assert_eq!(
        again["errors"][0]["extensions"]["code"], "VALIDATION",
        "{again}"
    );
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn pull_009_a_removal_deletes_the_copy_on_its_machine(pool: sqlx::PgPool) {
    seed_library(&pool).await;
    let work = Work::new(&pool).await;
    let token = work.daemons.token_for(PVE_01).await;
    let (status, _) = work.claim(&token, &model_claim(&json!([]))).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "the hub learns the machine");
    let address = format!("qwen3:{TAG}");
    let copy = pull(&work, &address).await["data"]["pullModel"]["id"].clone();
    let finished = done(&copy, "downloaded");
    run(
        &work,
        &token,
        &["preparing", "running", "finishing", "done"],
        finished,
    )
    .await;

    let removed = work.daemons.operate(REMOVE, json!({ "id": copy })).await;
    assert_eq!(
        removed["data"]["removeModelCopy"]["state"], "REMOVING",
        "{removed}"
    );
    let (status, item) = work.claim(&token, &model_claim(&json!([]))).await;
    assert_eq!(status, StatusCode::OK, "{item}");
    assert_eq!(item["type"], "copy_delete");
    assert_eq!(item["parameters"], json!({ "copy": copy }));
    for state in ["preparing", "running", "finishing"] {
        work.report(&token, &report(&item, state, json!({}))).await;
    }
    let (status, body) = work
        .report(&token, &report(&item, "done", done(&copy, "failed")))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        copies(&work).await.is_empty(),
        "a removed copy is not listed"
    );

    // A pull that waits in the queue stops, and its copy goes at once.
    let waiting = pull(&work, &address).await["data"]["pullModel"]["id"].clone();
    assert_ne!(waiting, copy, "a removed copy is not pulled again");
    let removed = work.daemons.operate(REMOVE, json!({ "id": waiting })).await;
    assert_eq!(removed["data"]["removeModelCopy"], Value::Null, "{removed}");
    assert!(copies(&work).await.is_empty());
    let (status, _) = work.claim(&token, &model_claim(&json!([]))).await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "no item is left for the machine"
    );
}

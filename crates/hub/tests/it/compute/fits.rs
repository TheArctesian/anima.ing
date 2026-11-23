//! The fit of each variant on each machine (FIT-001 to FIT-005; compute plan task L1-20):
//! `modelFits` reads the library, the hardware values that each daemon sent and the memory
//! share of its device policy.

use axum::http::StatusCode;
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::daemon::{
    batches::cpu_batch,
    harness::{DOCKER_HOST, Daemons, PVE_01, pushing_now},
};

const FITS: &str = "query($model: String!, $filter: ModelFitFilter) {
    modelFits(model: $model, filter: $filter) { variant address place { kind id name } state
    placement backend contextTokens largestContext sizeBytes usableBytes reason } }";

const GIB: u64 = 1 << 30;

/// `qwen3` with the layer shape of Qwen3 8B, and the variants `large` (60 GiB) and `small`
/// (5 GiB) of one file each; the library keeps the variants in tag order.
async fn seed_library(pool: &PgPool) {
    let model: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO compute.models (id, name, context_length, layers, hidden_size, \
         attention_heads, key_value_heads, added_at) \
         VALUES (gen_random_uuid(), 'qwen3', 32768, 36, 4096, 32, 8, now()) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    for (tag, size) in [("small", 5 * GIB), ("large", 60 * GIB)] {
        sqlx::query(
            "INSERT INTO compute.variants (model_id, tag, format, quantisation, repository, \
             revision) VALUES ($1, $2, 'gguf', 'Q4_K_M', 'unsloth/Qwen3-8B-GGUF', $3)",
        )
        .bind(model)
        .bind(tag)
        .bind("a".repeat(40))
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO compute.variant_files (model_id, tag, path, size_bytes, sha256) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(model)
        .bind(tag)
        .bind(format!("{tag}.gguf"))
        .bind(i64::try_from(size).unwrap())
        .bind("b".repeat(64))
        .execute(pool)
        .await
        .unwrap();
    }
}

/// `pve-01` has a CUDA GPU of 24 GiB and 64 GiB of system memory; `docker-host` has 16 GiB
/// and no GPU.
async fn machines(pool: &PgPool) -> Daemons {
    let now = pushing_now(pool).await;
    let daemons = Daemons::at(pool, now).await;
    let sent = [
        (
            PVE_01,
            json!({ "systemMemoryBytes": 64 * GIB, "dedicatedMemoryBytes": 24 * GIB,
            "cpuFeatures": [], "backends": [{ "name": "cuda" }, { "name": "cpu" }] }),
        ),
        (
            DOCKER_HOST,
            json!({ "systemMemoryBytes": 16 * GIB, "cpuFeatures": [],
            "backends": [{ "name": "cpu" }] }),
        ),
    ];
    for (device, hardware) in sent {
        let token = daemons.token_for(device).await;
        let mut batch = cpu_batch(device, now - TimeDelta::seconds(10), 5.0);
        batch["hardware"] = hardware;
        assert_eq!(daemons.push(&token, &batch).await.0, StatusCode::NO_CONTENT);
    }
    daemons
}

async fn fits(daemons: &Daemons, model: &str, filter: &Value) -> Value {
    let variables = json!({ "model": model, "filter": filter });
    daemons.operate(FITS, variables).await
}

fn row(answer: &Value, place: &str, variant: &str) -> Value {
    answer["data"]["modelFits"]
        .as_array()
        .unwrap_or_else(|| panic!("{answer}"))
        .iter()
        .find(|fit| fit["place"]["name"] == place && fit["variant"] == variant)
        .cloned()
        .unwrap_or_else(|| panic!("{place} {variant} in {answer}"))
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn fit_005_each_variant_has_a_verdict_on_each_machine_that_sent_its_hardware(pool: PgPool) {
    seed_library(&pool).await;
    let daemons = machines(&pool).await;

    let answer = fits(&daemons, "qwen3", &json!(null)).await;

    let order: Vec<(&str, &str)> = answer["data"]["modelFits"]
        .as_array()
        .unwrap_or_else(|| panic!("{answer}"))
        .iter()
        .map(|fit| {
            (
                fit["place"]["name"].as_str().unwrap(),
                fit["variant"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        order,
        [
            ("docker-host", "large"),
            ("docker-host", "small"),
            ("pve-01", "large"),
            ("pve-01", "small"),
        ]
    );
    let on_gpu = row(&answer, "pve-01", "small");
    assert_eq!(
        (&on_gpu["state"], &on_gpu["placement"], &on_gpu["backend"]),
        (&json!("FITS"), &json!("GPU"), &json!("cuda"))
    );
    assert_eq!(on_gpu["address"], "qwen3:small");
    assert_eq!(on_gpu["contextTokens"], 8192);
    assert_eq!(
        on_gpu["place"],
        json!({ "kind": "MACHINE", "id": PVE_01, "name": "pve-01" })
    );
    let partial = row(&answer, "pve-01", "large");
    assert_eq!(
        (&partial["state"], &partial["placement"]),
        (&json!("FITS"), &json!("PARTIAL"))
    );
    let on_cpu = row(&answer, "docker-host", "small");
    assert_eq!(
        (&on_cpu["state"], &on_cpu["placement"]),
        (&json!("FITS"), &json!("CPU"))
    );
    let too_large = row(&answer, "docker-host", "large");
    assert_eq!(too_large["state"], "TOO_LARGE", "{too_large}");
    assert!(
        too_large["reason"].as_str().unwrap().starts_with("Needs "),
        "{too_large}"
    );
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn fit_004_a_tag_a_machine_and_a_context_narrow_the_answer(pool: PgPool) {
    seed_library(&pool).await;
    let daemons = machines(&pool).await;

    let one = fits(
        &daemons,
        "qwen3:small",
        &json!({ "deviceId": DOCKER_HOST, "contextTokens": 100_000 }),
    )
    .await;

    let fits = one["data"]["modelFits"]
        .as_array()
        .unwrap_or_else(|| panic!("{one}"));
    assert_eq!(fits.len(), 1, "{one}");
    assert_eq!(
        fits[0]["contextTokens"], 32768,
        "the context of the model is the most"
    );
    assert_eq!(fits[0]["usableBytes"], json!(17_179_869_184.0));
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn fit_001_an_unknown_model_tag_or_context_is_refused(pool: PgPool) {
    seed_library(&pool).await;
    let daemons = machines(&pool).await;

    let no_model = fits(&daemons, "llama9", &json!(null)).await;
    let no_tag = fits(&daemons, "qwen3:huge", &json!(null)).await;
    let no_context = fits(&daemons, "qwen3", &json!({ "contextTokens": 0 })).await;

    assert_eq!(
        no_model["errors"][0]["extensions"]["code"], "NOT_FOUND",
        "{no_model}"
    );
    assert_eq!(
        no_tag["errors"][0]["extensions"]["code"], "VALIDATION",
        "{no_tag}"
    );
    assert_eq!(
        no_context["errors"][0]["extensions"]["code"], "VALIDATION",
        "{no_context}"
    );
}

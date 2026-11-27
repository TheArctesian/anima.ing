//! The completed-attempt image endpoint independently verifies what the store retained.
use http_body_util::BodyExt as _;
use sha2::{Digest as _, Sha256};
use uuid::Uuid;

use super::{
    Body, PERMITTED, PgPool, Receipt, Request, StatusCode, Store, Value, Work,
    downloads::{artifact_routes, read, send},
    header, json, offered, ready, report_of, upload,
};

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn only_a_permitted_operator_gets_the_verified_image_of_the_completed_attempt(pool: PgPool) {
    let work = Work::new(&pool).await;
    let (token, job) = ready(&work).await;
    let temporary = tempfile::tempdir().unwrap();
    let store = Store::open(&temporary.path().join("artifacts"), 1 << 20, 1)
        .unwrap()
        .with_registry(&anima_hub::config::ArtifactRegistry {
            origin: "https://registry.example.invalid".to_owned(),
            repository: "builds/private".to_owned(),
            authorization: None,
        })
        .unwrap();
    let app = artifact_routes(&work, &pool, store);
    let operator = work.daemons.operator.as_str();
    let (archive, manifest) = image_archive();
    let mut offer = offered(&job);
    offer.bytes = archive.len() as u64;
    offer.archive_digest = digest(&archive);
    offer.manifest_digest = manifest.clone();

    let sent = send(&app, upload(&token, &offer, Body::from(archive)), PERMITTED).await;
    assert_eq!(sent.status(), StatusCode::OK);
    let receipt: Receipt =
        serde_json::from_slice(&sent.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let uri = format!(
        "/builds/v1/source-artifacts/{}/{}/image",
        offer.job, offer.attempt
    );
    assert_eq!(
        read(&app, &uri, operator, false).await.status(),
        StatusCode::NOT_FOUND
    );
    let done = Request::post("/work/v1/report")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(report_of(&job, "done").to_string()))
        .unwrap();
    assert_eq!(send(&app, done, PERMITTED).await.status(), StatusCode::OK);

    assert_denied(&app, &uri, operator).await;
    let response = read(&app, &uri, operator, false).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "private, no-store"
    );
    let result: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        result,
        json!({
            "job": offer.job, "attempt": offer.attempt, "receipt": receipt.id,
            "archiveDigest": offer.archive_digest,
            "image": {"digest": manifest, "mediaType": "application/vnd.oci.image.manifest.v1+json",
              "platforms": [{"os":"linux", "architecture":"amd64"}]}
        })
    );
    assert_publication_fenced(&app, &pool, operator, &receipt).await;
}

async fn assert_publication_fenced(
    app: &axum::Router,
    pool: &PgPool,
    operator: &str,
    receipt: &Receipt,
) {
    let path = format!(
        "/builds/v1/source-artifacts/{}/{}/publish",
        receipt.artifact.job, receipt.artifact.attempt
    );
    let request = |id| {
        Request::post(&path)
            .header(header::AUTHORIZATION, format!("Bearer {operator}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "receipt": id,
                    "imageDigest": receipt.artifact.manifest_digest,
                })
                .to_string(),
            ))
            .unwrap()
    };
    let public = send(app, request(receipt.id), crate::daemon::harness::OUTSIDE).await;
    assert_eq!(public.status(), StatusCode::FORBIDDEN);
    let mismatch = send(app, request(Uuid::now_v7()), PERMITTED).await;
    assert_eq!(mismatch.status(), StatusCode::CONFLICT);
    let audits: i64 = sqlx::query_scalar(
        "select count(*) from events.audit_log where target_type = 'job' and target_id = $1 and change->>'operation' = 'publish_source_image'",
    ).bind(receipt.artifact.job.to_string()).fetch_one(pool).await.unwrap();
    assert_eq!(audits, 0);
}

async fn assert_denied(app: &axum::Router, uri: &str, operator: &str) {
    let outside = Request::get(uri)
        .header(header::AUTHORIZATION, format!("Bearer {operator}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        send(app, outside, crate::daemon::harness::OUTSIDE)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        read(app, uri, operator, true).await.status(),
        StatusCode::FORBIDDEN
    );
}

fn image_archive() -> (Vec<u8>, String) {
    let config =
        br#"{"os":"linux","architecture":"amd64","rootfs":{"diff_ids":[],"type":"layers"}}"#;
    let config_digest = digest(config);
    let manifest = serde_json::to_vec(&json!({
        "schemaVersion": 2,
        "config": {"mediaType":"application/vnd.oci.image.config.v1+json","digest":config_digest,"size":config.len()},
        "layers": []
    })).unwrap();
    let manifest_digest = digest(&manifest);
    let index = serde_json::to_vec(&json!({
        "schemaVersion": 2,
        "manifests": [{"mediaType":"application/vnd.oci.image.manifest.v1+json",
           "digest":manifest_digest,"size":manifest.len()}]
    }))
    .unwrap();
    let mut archive = tar::Builder::new(Vec::new());
    for (path, bytes) in [
        (
            "oci-layout".to_owned(),
            br#"{"imageLayoutVersion":"1.0.0"}"#.as_slice(),
        ),
        ("index.json".to_owned(), index.as_slice()),
        (
            format!("blobs/sha256/{}", &config_digest[7..]),
            config.as_slice(),
        ),
        (
            format!("blobs/sha256/{}", &manifest_digest[7..]),
            manifest.as_slice(),
        ),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive.append_data(&mut header, path, bytes).unwrap();
    }
    (archive.into_inner().unwrap(), manifest_digest)
}

fn digest(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = "sha256:".to_owned();
    for byte in Sha256::digest(bytes) {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

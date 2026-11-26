use axum::{
    Json, Router,
    extract::Path,
    http::{HeaderMap, header},
    response::IntoResponse as _,
    routing::{get, post},
};
use std::path::Path as LocalPath;
use tokio::net::TcpListener;

use super::signed_in;

const JOB: &str = "0190a000-0000-7000-8002-000000000001";
const RECEIPT: &str = "0190a000-0000-7000-8002-000000000002";
const SHA256: &str = "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

#[tokio::test]
async fn source_download_publishes_only_verified_bytes_to_a_new_file() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new().route("/builds/v1/source-artifacts/{job}/{attempt}", get(archive));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.oci");
    let rejected = download(&origin, &path, "1").await;
    assert_ne!(rejected.code, 0, "{rejected:?}");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    let accepted = download(&origin, &path, "2").await;
    assert_eq!(accepted.code, 0, "{accepted:?}");
    assert_eq!(std::fs::read(&path).unwrap(), b"abc");
    let record: serde_json::Value = serde_json::from_str(&accepted.stdout).unwrap();
    assert_eq!(record["artifact"]["receipt"], RECEIPT);
    assert_eq!(record["artifact"]["archiveDigest"], SHA256);
    let conflict = download(&origin, &path, "1").await;
    assert_eq!(conflict.code, 6, "{conflict:?}");
    assert_eq!(std::fs::read(&path).unwrap(), b"abc");
    server.abort();
}

async fn download(origin: &str, path: &LocalPath, attempt: &str) -> crate::kit::Outcome {
    signed_in()
        .run(&[
            "project",
            "build",
            "download",
            JOB,
            attempt,
            "--output",
            path.to_str().unwrap(),
            "--hub",
            origin,
            "--json",
        ])
        .await
}

async fn archive(
    Path((job, attempt)): Path<(String, String)>,
    headers: HeaderMap,
) -> impl axum::response::IntoResponse {
    assert_eq!(job, JOB);
    assert_eq!(
        headers[header::AUTHORIZATION],
        "Bearer login-token-for-project-tests"
    );
    let bytes = if attempt == "1" { "bad" } else { "abc" };
    (
        [
            ("content-length", "3"),
            ("anima-artifact-job", JOB),
            ("anima-artifact-attempt", &attempt),
            ("anima-artifact-receipt", RECEIPT),
            ("anima-artifact-archive", SHA256),
            ("anima-artifact-manifest", SHA256),
        ],
        bytes,
    )
        .into_response()
}

#[tokio::test]
async fn source_image_inspection_requires_the_same_job_and_attempt() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new().route(
        "/builds/v1/source-artifacts/{job}/{attempt}/image",
        get(image),
    );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let accepted = inspect(&origin, "2").await;
    assert_eq!(accepted.code, 0, "{accepted:?}");
    let result: serde_json::Value = serde_json::from_str(&accepted.stdout).unwrap();
    assert_eq!(result["receipt"], RECEIPT);
    assert_eq!(result["image"]["digest"], SHA256);
    assert_eq!(result["image"]["platforms"][0]["architecture"], "amd64");
    let mismatched = inspect(&origin, "1").await;
    assert_ne!(mismatched.code, 0, "{mismatched:?}");
    assert!(!mismatched.stdout.contains(SHA256));
    server.abort();
}

async fn inspect(origin: &str, attempt: &str) -> crate::kit::Outcome {
    signed_in()
        .run(&[
            "project",
            "build",
            "inspect-image",
            JOB,
            attempt,
            "--hub",
            origin,
            "--json",
        ])
        .await
}

async fn image(
    Path((job, _)): Path<(String, u32)>,
    headers: HeaderMap,
) -> axum::Json<serde_json::Value> {
    assert_eq!(job, JOB);
    assert_eq!(
        headers[header::AUTHORIZATION],
        "Bearer login-token-for-project-tests"
    );
    axum::Json(serde_json::json!({
        "job": JOB, "attempt": 2, "receipt": RECEIPT, "archiveDigest": SHA256,
        "image": {"digest": SHA256, "mediaType": "application/vnd.oci.image.manifest.v1+json",
          "platforms": [{"os": "linux", "architecture": "amd64"}]}
    }))
}

#[tokio::test]
async fn source_image_publication_binds_confirmation_and_the_returned_reference() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new().route(
        "/builds/v1/source-artifacts/{job}/{attempt}/publish",
        post(publish),
    );
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let accepted = publish_command(&origin, "2").await;
    assert_eq!(accepted.code, 0, "{accepted:?}");
    let result: serde_json::Value = serde_json::from_str(&accepted.stdout).unwrap();
    assert_eq!(
        result["reference"],
        format!("registry.example.invalid/builds/private@{SHA256}")
    );
    let mutable_reference = publish_command(&origin, "3").await;
    assert_ne!(mutable_reference.code, 0, "{mutable_reference:?}");
    assert_eq!(mutable_reference.stdout, "");
    server.abort();
}

async fn publish_command(origin: &str, attempt: &str) -> crate::kit::Outcome {
    signed_in()
        .run(&[
            "project",
            "build",
            "publish-image",
            JOB,
            attempt,
            "--receipt",
            RECEIPT,
            "--digest",
            SHA256,
            "--hub",
            origin,
            "--json",
        ])
        .await
}

async fn publish(
    Path((job, attempt)): Path<(String, u32)>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    use serde_json::json;
    assert_eq!(job, JOB);
    assert_eq!(
        headers[header::AUTHORIZATION],
        "Bearer login-token-for-project-tests"
    );
    assert_eq!(input, json!({"receipt":RECEIPT, "imageDigest":SHA256}));
    let reference = if attempt == 2 {
        format!("registry.example.invalid/builds/private@{SHA256}")
    } else {
        "registry.example.invalid/builds/private:latest".to_owned()
    };
    Json(json!({
        "job": JOB, "attempt": attempt, "receipt": RECEIPT,
        "imageDigest": SHA256, "reference": reference,
    }))
}

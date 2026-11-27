use std::{
    io::{Seek as _, Write as _},
    sync::Mutex,
};

use axum::{
    Router,
    body::Bytes,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode, Uri, header},
    middleware::{Next, from_fn},
    response::{IntoResponse as _, Response},
    routing::{get, head, post, put},
};
use oci_artifact::{Image, Part};
use sha2::{Digest as _, Sha256};

use super::*;
use crate::work::artifacts::model::encoded_digest;

const BASE: &str = "/v2/builds/private";
const MEDIA: &str = "application/vnd.oci.image.manifest.v1+json";
const BEARER: &str = "Bearer fixture-registry-write";
const BLOB: &[u8] = b"fixture image configuration";
const MANIFEST: &[u8] = br#"{"schemaVersion":2,"layers":[]}"#;

#[tokio::test]
async fn sends_archive_ranges_by_digest_and_replays_without_rewriting_objects() {
    let (publisher, server, registry) = server(None).await;
    let (archive, plan) = archive();
    for _ in 0..2 {
        publisher
            .push(archive.reopen().unwrap(), &plan)
            .await
            .unwrap();
    }
    let authority = publisher.transport.reference(&plan.image.digest);
    assert!(authority.ends_with(&format!("/builds/private@{}", plan.image.digest)));
    let saved = registry.lock().unwrap();
    assert_eq!(saved.writes, ["blob", "manifest"]);
    assert_eq!(saved.blob, BLOB);
    assert_eq!(saved.manifest, MANIFEST);
    server.abort();
}

#[tokio::test]
async fn refuses_an_upload_session_outside_the_configured_repository() {
    let (publisher, server, registry) = server(Some("/v2/other/blobs/uploads/new")).await;
    let (archive, plan) = archive();
    assert!(matches!(
        publisher.push(archive.reopen().unwrap(), &plan).await,
        Err(Error::Registry)
    ));
    assert_eq!(registry.lock().unwrap().writes, Vec::<&str>::new());
    server.abort();
}

fn archive() -> (tempfile::NamedTempFile, Publication) {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let mut parts = Vec::new();
    for (bytes, media_type) in [(BLOB, None), (MANIFEST, Some(MEDIA.to_owned()))] {
        file.write_all(b"archive padding").unwrap();
        parts.push(Part {
            digest: digest(bytes),
            offset: file.stream_position().unwrap(),
            size: bytes.len() as u64,
            media_type,
        });
        file.write_all(bytes).unwrap();
    }
    let plan = Publication {
        image: Image {
            digest: parts[1].digest.clone(),
            media_type: MEDIA.to_owned(),
            platforms: Vec::new(),
        },
        parts,
    };
    (file, plan)
}

#[derive(Default)]
struct Registry {
    upload: Option<&'static str>,
    blob: Vec<u8>,
    manifest: Vec<u8>,
    writes: Vec<&'static str>,
}

async fn server(
    upload: Option<&'static str>,
) -> (Publisher, tokio::task::JoinHandle<()>, Arc<Mutex<Registry>>) {
    let state = Arc::new(Mutex::new(Registry {
        upload,
        ..Registry::default()
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}/", listener.local_addr().unwrap())
        .parse()
        .unwrap();
    let app = Router::new()
        .route(&format!("{BASE}/blobs/uploads/"), post(open_upload))
        .route(&format!("{BASE}/blobs/uploads/session"), put(write_blob))
        .route(&format!("{BASE}/blobs/{{digest}}"), head(read_blob))
        .route(
            &format!("{BASE}/manifests/{{digest}}"),
            get(read_manifest).put(write_manifest),
        )
        .fallback(unexpected)
        .layer(from_fn(authenticated))
        .with_state(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let publisher = Publisher::new(
        Destination {
            origin,
            repository: "builds/private".to_owned(),
        },
        Some(HeaderValue::from_static(BEARER)),
    )
    .unwrap();
    (publisher, task, state)
}

type RegistryState = State<Arc<Mutex<Registry>>>;

async fn authenticated(request: Request, next: Next) -> Response {
    assert_eq!(request.headers()[header::AUTHORIZATION], BEARER);
    next.run(request).await
}

async fn open_upload(State(registry): RegistryState, body: Bytes) -> Response {
    assert_eq!(body, b""[..]);
    let location = registry
        .lock()
        .unwrap()
        .upload
        .unwrap_or("session?_state=saved");
    (StatusCode::ACCEPTED, [(header::LOCATION, location)]).into_response()
}

async fn read_blob(State(registry): RegistryState, Path(name): Path<String>) -> Response {
    assert_eq!(name, digest(BLOB));
    let state = registry.lock().unwrap();
    if state.blob.is_empty() {
        return StatusCode::NOT_FOUND.into_response();
    }
    (
        [
            ("docker-content-digest", name),
            ("content-length", state.blob.len().to_string()),
        ],
        (),
    )
        .into_response()
}

async fn write_blob(
    State(registry): RegistryState,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let blob = digest(BLOB);
    assert_eq!(
        uri.query().unwrap(),
        format!("_state=saved&digest={}", blob.replace(':', "%3A"))
    );
    assert_eq!(headers[header::CONTENT_TYPE], "application/octet-stream");
    assert_eq!(body, BLOB);
    let mut state = registry.lock().unwrap();
    state.blob = body.to_vec();
    state.writes.push("blob");
    (StatusCode::CREATED, [("docker-content-digest", blob)]).into_response()
}

async fn write_manifest(
    State(registry): RegistryState,
    Path(name): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    assert_eq!(name, digest(MANIFEST));
    assert_eq!(headers[header::CONTENT_TYPE], MEDIA);
    assert_eq!(body, MANIFEST);
    let mut state = registry.lock().unwrap();
    assert_eq!(state.blob, BLOB);
    state.manifest = body.to_vec();
    state.writes.push("manifest");
    (StatusCode::CREATED, [("docker-content-digest", name)]).into_response()
}

async fn read_manifest(
    State(registry): RegistryState,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Response {
    assert_eq!(name, digest(MANIFEST));
    assert_eq!(headers[header::ACCEPT], MEDIA);
    let state = registry.lock().unwrap();
    if state.manifest.is_empty() {
        return StatusCode::NOT_FOUND.into_response();
    }
    (
        [
            ("docker-content-digest", name.as_str()),
            ("content-type", MEDIA),
        ],
        state.manifest.clone(),
    )
        .into_response()
}

async fn unexpected(State(registry): RegistryState) -> StatusCode {
    registry.lock().unwrap().writes.push("unexpected");
    StatusCode::BAD_REQUEST
}

fn digest(bytes: &[u8]) -> String {
    encoded_digest(&Sha256::digest(bytes))
}

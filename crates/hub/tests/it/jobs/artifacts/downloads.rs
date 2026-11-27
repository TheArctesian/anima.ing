use std::sync::Arc;

use anima_hub::{
    all_routes_with_artifacts,
    fleet::{access::Edge, events::Bus},
};
use axum::{Router, body::Body, extract::ConnectInfo, http::header};
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;

use super::{
    PERMITTED, PgPool, Receipt, Request, StatusCode, Store, Work, offered, ready, report_of, upload,
};

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn source_download_requires_the_right_operator_and_completed_attempt(pool: PgPool) {
    let work = Work::new(&pool).await;
    let (token, job) = ready(&work).await;
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("artifacts");
    let store = Store::open(&root, 6, 1).unwrap();
    let offer = offered(&job);
    let app = artifact_routes(&work, &pool, store);
    let operator = work.daemons.operator.as_str();
    let uri = format!(
        "/builds/v1/source-artifacts/{}/{}",
        offer.job, offer.attempt
    );

    let sent = send(&app, upload(&token, &offer, Body::from("abc")), PERMITTED).await;
    assert_eq!(sent.status(), StatusCode::OK);
    let receipt: Receipt =
        serde_json::from_slice(&sent.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        read(&app, &uri, operator, false).await.status(),
        StatusCode::NOT_FOUND
    );
    let done = Request::post("/work/v1/report")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(report_of(&job, "done").to_string()))
        .unwrap();
    assert_eq!(send(&app, done, PERMITTED).await.status(), StatusCode::OK);

    let denied = read(&app, &uri, &token, false).await;
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read(&app, &uri, operator, true).await.status(),
        StatusCode::FORBIDDEN
    );
    let older = format!("/builds/v1/source-artifacts/{}/0", offer.job);
    assert_eq!(
        read(&app, &older, operator, false).await.status(),
        StatusCode::NOT_FOUND
    );
    verify_download(&app, &uri, operator, &receipt).await;
    verify_invalid_image(&app, &uri, operator).await;
    let audit: Vec<serde_json::Value> = sqlx::query_scalar(
        "select change from events.audit_log where target_type = 'job' and target_id = $1 and action = 'download'",
    ).bind(offer.job.to_string()).fetch_all(&pool).await.unwrap();
    assert_eq!(
        audit,
        [serde_json::json!({"attempt": offer.attempt, "receipt": receipt.id})]
    );

    let path = root.join(format!("{}-{}/archive.oci", offer.job, offer.attempt));
    std::fs::write(path, b"bad").unwrap();
    assert_eq!(
        read(&app, &uri, operator, false).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

async fn verify_invalid_image(app: &Router, uri: &str, operator: &str) {
    assert_eq!(
        read(app, &format!("{uri}/image"), operator, false)
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

pub(super) fn artifact_routes(work: &Work, pool: &PgPool, store: Store) -> Router {
    let bus = Bus::new(16);
    let clock: Arc<dyn anima_hub::clock::Clock> = work.daemons.clock.clone();
    let edge = Edge::new(pool.clone(), bus.clone(), clock.clone());
    all_routes_with_artifacts(
        crate::hub::dependencies(pool, clock, bus),
        edge,
        Some(store),
    )
}

async fn verify_download(app: &Router, uri: &str, operator: &str, receipt: &Receipt) {
    let outside = Request::get(uri)
        .header(header::AUTHORIZATION, format!("Bearer {operator}"))
        .body(Body::empty())
        .unwrap();
    let denied = send(app, outside, crate::daemon::harness::OUTSIDE).await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert!(
        !denied
            .headers()
            .contains_key(daemon_core::artifact::RECEIPT)
    );
    let bytes = denied.into_body().collect().await.unwrap().to_bytes();
    assert!(!bytes.windows(3).any(|chunk| chunk == b"abc"));

    let output = read(app, uri, operator, false).await;
    assert_eq!(output.status(), StatusCode::OK);
    assert_eq!(output.headers()[header::CACHE_CONTROL], "private, no-store");
    assert_eq!(output.headers()[header::CONTENT_LENGTH], "3");
    assert_eq!(
        output.headers()[daemon_core::artifact::RECEIPT],
        receipt.id.to_string()
    );
    assert_eq!(
        output.headers()[daemon_core::artifact::MANIFEST],
        receipt.artifact.manifest_digest
    );
    assert_eq!(
        output.into_body().collect().await.unwrap().to_bytes(),
        "abc"
    );
}

pub(super) async fn read(
    app: &Router,
    uri: &str,
    bearer: &str,
    agent: bool,
) -> axum::response::Response {
    let mut request = Request::get(uri).header(header::AUTHORIZATION, format!("Bearer {bearer}"));
    if agent {
        request = request.header("anima-actor", "agent");
    }
    send(app, request.body(Body::empty()).unwrap(), PERMITTED).await
}

pub(super) async fn send(
    app: &Router,
    mut request: Request<Body>,
    from: &str,
) -> axum::response::Response {
    request.extensions_mut().insert(ConnectInfo(
        format!("{from}:50000")
            .parse::<std::net::SocketAddr>()
            .unwrap(),
    ));
    app.clone().oneshot(request).await.unwrap()
}

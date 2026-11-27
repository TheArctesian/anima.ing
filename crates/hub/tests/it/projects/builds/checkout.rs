//! Private source jobs through Projects, K10 and the connected loopback GitHub App.
mod fixture;
mod workspace;

use std::{sync::Arc, time::Duration};

use anima_hub::work::checkout;
use axum::http::{StatusCode, header};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::Semaphore;

use self::fixture::{CHECKOUT, Fixture, MINT, REVOKE};
use crate::sources::stub::Reply;

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn a_private_build_releases_only_once_on_its_capable_project_pin_and_revokes_on_end(
    pool: PgPool,
) {
    let fixture = Fixture::new(&pool).await;
    let job = fixture.claim_private().await;
    let claimed = fixture.secret(&fixture.primary, &job).await;
    assert_eq!(claimed.status(), StatusCode::CONFLICT);
    let report = fixture.report(&job, "preparing").await;
    let lease: DateTime<Utc> = report["lease"]["until"].as_str().unwrap().parse().unwrap();
    let other = fixture.secret(&fixture.secondary, &job).await;
    assert_eq!(other.status(), StatusCode::CONFLICT);
    let mut previous = job.clone();
    previous["attempt"] = json!(0);
    assert_eq!(
        fixture.secret(&fixture.primary, &previous).await.status(),
        StatusCode::CONFLICT
    );
    let before = fixture.kit.stub.paths().len();
    fixture.mint_reply();
    let released = fixture.secret(&fixture.primary, &job).await;
    assert_eq!(released.status(), StatusCode::OK);
    assert_eq!(released.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        fixture::json_body(released).await,
        json!({"value":CHECKOUT,"expiresAt":lease})
    );
    let repeated = fixture.secret(&fixture.primary, &job).await;
    assert_eq!(repeated.status(), StatusCode::CONFLICT);
    assert_eq!(
        fixture.audit(&job).await,
        [json!({
            "attempt":job["attempt"], "device":fixture::PVE_01,
            "purpose":"repository_token", "project":fixture.project
        })]
    );
    checkout::revoke_due(&fixture.scheduling).await.unwrap();
    assert_eq!(&fixture.kit.stub.paths()[before..], [MINT]);
    fixture.report(&job, "failed").await;
    checkout::revoke_due(&fixture.scheduling).await.unwrap();
    checkout::revoke_due(&fixture.scheduling).await.unwrap();
    fixture.assert_revoked(before);
    let cleared = fixture.set_pin(None).await;
    assert!(cleared["errors"].is_null(), "{cleared}");
    assert_eq!(cleared["data"]["setProjectBuildPin"]["pinned"], json!(null));
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn a_lease_can_report_during_mint_and_expiry_prevents_release(pool: PgPool) {
    let fixture = Fixture::new(&pool).await;
    let job = fixture.claim_private().await;
    fixture.report(&job, "preparing").await;
    let before = fixture.kit.stub.paths().len();
    fixture.mint_reply();
    let gate = Arc::new(Semaphore::new(0));
    fixture.kit.stub.gate(MINT, &gate);
    let (app, request) = (
        fixture.app.clone(),
        Fixture::secret_request(&fixture.primary, &job),
    );
    let pending = tokio::spawn(async move { fixture::send(&app, request).await });
    tokio::time::timeout(Duration::from_secs(4), async {
        while fixture.kit.stub.paths().len() == before {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    fixture.report(&job, "running").await;
    assert_eq!(
        fixture.secret(&fixture.primary, &job).await.status(),
        StatusCode::CONFLICT
    );
    fixture.clock.advance(TimeDelta::minutes(5));
    gate.add_permits(1);
    let denied = pending.await.unwrap();
    assert_eq!(denied.status(), StatusCode::CONFLICT);
    assert_eq!(
        fixture::json_body(denied).await,
        json!({"error":"SECRET_REFUSED"})
    );
    assert_eq!(fixture.audit(&job).await, Vec::<Value>::new());

    fixture.kit.stub.reply_once(
        "DELETE",
        REVOKE,
        Reply {
            status: 503,
            headers: vec![],
            body: "{}".to_owned(),
        },
    );
    checkout::revoke_due(&fixture.scheduling).await.unwrap();
    checkout::revoke_due(&fixture.scheduling).await.unwrap();
    assert_eq!(&fixture.kit.stub.paths()[before..], [MINT, REVOKE]);
    fixture.clock.advance(TimeDelta::minutes(1));
    checkout::revoke_due(&fixture.scheduling).await.unwrap();
    checkout::revoke_due(&fixture.scheduling).await.unwrap();
    assert_eq!(&fixture.kit.stub.paths()[before..], [MINT, REVOKE, REVOKE]);
}

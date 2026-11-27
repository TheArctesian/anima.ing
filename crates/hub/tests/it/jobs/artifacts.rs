mod downloads;
mod images;
mod publication;

use super::{Work, build, claim_of, report_of};
use crate::daemon::harness::{Daemons, PERMITTED, PVE_01};
use anima_hub::{
    fleet::events::{ClientAddr, Operator},
    ids::JobId,
    work::{
        artifacts::{Error, Intake, Offer, Receipt, Store},
        jobs,
    },
};
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use chrono::TimeDelta;
use daemon_core::wire::StopReason;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWriteExt, DuplexStream, ReadBuf},
    sync::oneshot,
    task::JoinHandle,
};

struct ObservedInput {
    stream: DuplexStream,
    reading: Option<oneshot::Sender<()>>,
}

impl AsyncRead for ObservedInput {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if let Some(reading) = self.reading.take() {
            let _ = reading.send(());
        }
        Pin::new(&mut self.stream).poll_read(context, bytes)
    }
}

async fn ready(work: &Work) -> (String, Value) {
    let token = work.daemons.token_for(PVE_01).await;
    let mut source = build();
    source.parameters.as_object_mut().unwrap().remove("version");
    source.parameters["recipe"] = json!("source_image_v1");
    source.parameters["strategy"] = json!("auto");
    work.enqueue(source).await;
    let mut claim = claim_of(PVE_01, &json!([]));
    claim["buildRecipes"] = json!(["source_image_v1"]);
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(&directory.path().join("artifacts"), 6, 1).unwrap();
    let daemons = work.daemons.clone().with_artifact_storage(store);
    let request = Request::post("/work/v1/claim")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(claim.to_string()))
        .unwrap();
    let (status, job) = daemons.send(request, PERMITTED).await;
    let job = serde_json::from_str(&job).unwrap();
    assert_eq!(status, StatusCode::OK);
    for state in ["preparing", "running", "finishing"] {
        assert_eq!(
            work.report(&token, &report_of(&job, state)).await.0,
            StatusCode::OK
        );
    }
    (token, job)
}

fn offered(job: &Value) -> Offer {
    Offer {
        job: job["id"].as_str().unwrap().parse().unwrap(),
        attempt: u32::try_from(job["attempt"].as_u64().unwrap()).unwrap(),
        bytes: 3,
        archive_digest: "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            .into(),
        manifest_digest: format!("sha256:{}", "1".repeat(64)),
    }
}

fn upload(token: &str, offer: &Offer, body: Body) -> Request<Body> {
    Request::post("/work/v1/artifact")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("anima-artifact-job", offer.job.to_string())
        .header("anima-artifact-attempt", offer.attempt)
        .header("anima-artifact-bytes", offer.bytes)
        .header("anima-artifact-archive", &offer.archive_digest)
        .header("anima-artifact-manifest", &offer.manifest_digest)
        .body(body)
        .unwrap()
}

async fn transferring(
    daemons: Daemons,
    token: &str,
    offer: &Offer,
) -> (JoinHandle<(StatusCode, String)>, DuplexStream) {
    let (writer, stream) = tokio::io::duplex(64);
    let (reading, began) = oneshot::channel();
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(ObservedInput {
        stream,
        reading: Some(reading),
    }));
    let request = upload(token, offer, body);
    let transfer = tokio::spawn(async move { daemons.send(request, PERMITTED).await });
    tokio::time::timeout(Duration::from_secs(5), began)
        .await
        .unwrap()
        .unwrap();
    (transfer, writer)
}

async fn not_retained(store: &Store, offer: Offer) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match store.begin(PVE_01.parse().unwrap(), offer.clone()).await {
                Err(Error::Busy) => tokio::task::yield_now().await,
                Ok(Intake::Upload(_)) => break,
                _ => panic!("unauthorized output was retained"),
            }
        }
    })
    .await
    .unwrap();
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn source_artifact_rechecks_expiry_and_stop_after_upload_without_blocking_reports(
    pool: PgPool,
) {
    let work = Work::new(&pool).await;
    for stop in [false, true] {
        let (token, job) = ready(&work).await;
        let temporary = tempfile::tempdir().unwrap();
        let store = Store::open(&temporary.path().join("artifacts"), 6, 1).unwrap();
        let offer = offered(&job);
        let daemons = work.daemons.clone().with_artifact_storage(store.clone());
        let (transfer, mut writer) = transferring(daemons.clone(), &token, &offer).await;
        let report = report_of(&job, "finishing");
        let (status, _) = tokio::time::timeout(
            Duration::from_secs(5),
            daemons.send(
                Request::post("/work/v1/report")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(report.to_string()))
                    .unwrap(),
                PERMITTED,
            ),
        )
        .await
        .unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "upload must not hold the lease-reporting row locks"
        );
        if stop {
            jobs::stop(
                work.queue(),
                (JobId::from(offer.job), StopReason::Operator),
                (Operator::hub(), ClientAddr("127.0.0.1".parse().unwrap())),
                work.now(),
            )
            .await
            .unwrap();
        } else {
            work.daemons.clock.advance(TimeDelta::seconds(91));
        }
        writer.write_all(b"abc").await.unwrap();
        writer.shutdown().await.unwrap();
        let result = tokio::time::timeout(Duration::from_secs(5), transfer)
            .await
            .unwrap()
            .unwrap();
        assert!(result.0 == StatusCode::CONFLICT, "{result:?}");
        not_retained(&store, offer).await;
    }
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn source_artifact_receipt_replay_requires_the_current_holder(pool: PgPool) {
    let work = Work::new(&pool).await;
    let (token, job) = ready(&work).await;
    let temporary = tempfile::tempdir().unwrap();
    let store = Store::open(&temporary.path().join("artifacts"), 6, 1).unwrap();
    let daemons = work.daemons.clone().with_artifact_storage(store);
    let offer = offered(&job);
    let (status, body) = work
        .daemons
        .send(upload(&token, &offer, Body::from("abc")), PERMITTED)
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    let (status, body) = daemons
        .send(upload("invalid", &offer, Body::from("abc")), PERMITTED)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    let (status, body) = daemons
        .send(upload(&token, &offer, Body::from("abc")), PERMITTED)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let receipt: Receipt = serde_json::from_str(&body).unwrap();
    assert_eq!(receipt.artifact, offer);
    assert_eq!(receipt.device, PVE_01.parse::<uuid::Uuid>().unwrap());
    let (status, body) = daemons
        .send(upload(&token, &offer, Body::empty()), PERMITTED)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(serde_json::from_str::<Receipt>(&body).unwrap(), receipt);
    work.daemons.clock.advance(TimeDelta::seconds(91));
    let (status, body) = daemons
        .send(upload(&token, &offer, Body::empty()), PERMITTED)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn source_final_report_replays_only_the_accepted_result(pool: PgPool) {
    let work = Work::new(&pool).await;
    let (token, job) = ready(&work).await;
    let report = report_of(&job, "failed");
    let (status, accepted) = work.report(&token, &report).await;
    assert_eq!(status, StatusCode::OK);
    work.daemons.clock.advance(TimeDelta::seconds(91));
    assert_eq!(work.report(&token, &report).await, (status, accepted));

    let mut changed = report.clone();
    changed["result"]["seconds"] = json!(8);
    assert_eq!(work.report(&token, &changed).await.0, StatusCode::CONFLICT);
    changed = report.clone();
    changed["attempt"] = json!(2);
    assert_eq!(work.report(&token, &changed).await.0, StatusCode::CONFLICT);
    changed = report;
    changed["state"] = json!("finishing");
    changed["result"] = Value::Null;
    assert_eq!(work.report(&token, &changed).await.0, StatusCode::CONFLICT);
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn source_success_requires_retained_output_for_the_same_attempt(pool: PgPool) {
    let work = Work::new(&pool).await;
    let (token, job) = ready(&work).await;
    let temporary = tempfile::tempdir().unwrap();
    let store = Store::open(&temporary.path().join("artifacts"), 6, 1).unwrap();
    let daemons = work.daemons.clone().with_artifact_storage(store);
    let done = report_of(&job, "done");
    let report = |body: &Value| {
        Request::post("/work/v1/report")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let premature = daemons.send(report(&done), PERMITTED).await;
    assert_eq!(premature.0, StatusCode::BAD_REQUEST, "{premature:?}");

    let offer = offered(&job);
    let upload = daemons
        .send(upload(&token, &offer, Body::from("abc")), PERMITTED)
        .await;
    assert_eq!(upload.0, StatusCode::OK, "{upload:?}");
    let mut incomplete = done.clone();
    incomplete["result"]["cleanup"]["checkout"] = json!("failed");
    let rejected = daemons.send(report(&incomplete), PERMITTED).await;
    assert_eq!(rejected.0, StatusCode::BAD_REQUEST, "{rejected:?}");
    let accepted = daemons.send(report(&done), PERMITTED).await;
    assert_eq!(accepted.0, StatusCode::OK, "{accepted:?}");
    assert_eq!(daemons.send(report(&done), PERMITTED).await, accepted);
}

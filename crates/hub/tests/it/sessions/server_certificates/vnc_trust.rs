//! VNC acceptance cannot consume an RDP observation, even at the same endpoint.
use super::{
    accept_request, point_at, result,
    servers::{Answer, Certificate, rdp, vnc_tls},
    test_request,
};
use crate::sessions::{
    PVE_01, add_operator,
    harness::{TestSchema, error_json},
    host_keys::{loopback_live, schema},
};
use async_graphql::Request;
use serde_json::{Value, json};
use sqlx::PgPool;

fn accept_vnc(fingerprint: &str) -> Request {
    let mut request = accept_request(PVE_01, fingerprint);
    request.query = request
        .query
        .replace("acceptServerCertificate", "acceptVncServerCertificate");
    request
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn vnc_observation_acceptance_and_storage_are_independent_of_rdp(pool: PgPool) {
    add_operator(&pool).await;
    let (rdp_cert, vnc_cert) = (Certificate::new("RDP"), Certificate::new("VNC"));
    let mut rdp_peer = rdp(Answer::Tls, &rdp_cert).await;
    let port = rdp_peer.port;
    point_at(&pool, port, "nla", false).await;
    let live = loopback_live();
    let schema = schema(&pool, &live);
    let rdp_input = json!({"protocol":"RDP", "deviceId":PVE_01});
    let observed = result(schema.execute(test_request(&rdp_input)).await);
    assert_eq!(
        observed["serverCertificate"]["fingerprintSha256"],
        rdp_cert.fingerprint()
    );
    assert_eq!(rdp_peer.visit().await.after, [] as [u8; 0]);
    conflict(&schema, accept_vnc(&rdp_cert.fingerprint())).await;
    accepted(&schema, accept_request(PVE_01, &rdp_cert.fingerprint())).await;
    drop(rdp_peer);
    tokio::task::yield_now().await;
    let mut vnc_peer = vnc_tls(&vnc_cert, port).await;
    let input = json!({"protocol":"VNC", "deviceId":PVE_01});
    let observed = result(schema.execute(test_request(&input)).await);
    assert_eq!(
        observed["serverCertificate"]["fingerprintSha256"],
        vnc_cert.fingerprint()
    );
    assert_eq!(observed["certificateChanged"], false);
    assert_eq!(observed["authenticated"], false);
    assert_eq!(vnc_peer.visit().await.after, [] as [u8; 0]);
    conflict(&schema, accept_request(PVE_01, &vnc_cert.fingerprint())).await;
    accepted(&schema, accept_vnc(&vnc_cert.fingerprint())).await;
    let matched = result(schema.execute(test_request(&input)).await);
    assert_eq!(matched["certificateMatched"], true);
    assert_eq!(matched["serverCertificate"], Value::Null);
    assert_eq!(vnc_peer.visit().await.after, [] as [u8; 0]);
    let id = PVE_01.parse().unwrap();
    let rdp = anima_hub::fleet::trust::server_certificates(&pool, &[id])
        .await
        .unwrap();
    let vnc = anima_hub::fleet::trust::vnc_server_certificates(&pool, &[id])
        .await
        .unwrap();
    assert_eq!(rdp[&id].fingerprint_sha256, rdp_cert.fingerprint());
    assert_eq!(vnc[&id].fingerprint_sha256, vnc_cert.fingerprint());
}

async fn conflict(schema: &TestSchema, request: Request) {
    let response = schema.execute(request).await;
    assert_eq!(error_json(&response)["extensions"]["code"], "CONFLICT");
}

async fn accepted(schema: &TestSchema, request: Request) {
    let response = schema.execute(request).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
}

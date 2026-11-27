use super::super::{
    Bench, Setup, bench,
    bench::VNC_PASSWORD,
    decoded,
    fake_guacd::{Act, Script, VNC_ARGS, fake_guacd},
    opened,
    records::{DOCKER_HOST, LOOPBACK, ended, ended_once},
    socket::{open, open_with},
};
use super::{
    first_frame, native_gateway,
    vnc_tls::{self, Device, Visit},
};
use crate::fleet::sessions::Protocol;
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::timeout;

async fn pinned(pool: &PgPool, device: &Device, fingerprint: &str, gateway: &str) -> Bench {
    let bench = bench(pool, gateway, Setup::protocol(Protocol::Vnc)).await;
    sqlx::query("update devices.vnc_settings set port = $2 where device_id = $1::text::uuid")
        .bind(DOCKER_HOST)
        .bind(i32::from(device.address.port()))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("insert into trust.vnc_server_certificates (device_id, fingerprint_sha256, first_seen_at, accepted_by) select device_id, $2, first_seen_at, accepted_by from trust.server_certificates where device_id = $1::text::uuid")
        .bind(DOCKER_HOST).bind(fingerprint).execute(pool).await.unwrap();
    bench
}

async fn visit(device: &mut Device) -> Visit {
    timeout(Duration::from_secs(5), device.visits.recv())
        .await
        .unwrap()
        .unwrap()
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../../../tests/fixtures", scripts("devices"))
)]
async fn pinned_vnc_opens_over_tls(pool: PgPool) {
    let mut device = vnc_tls::device().await;
    let bench = pinned(&pool, &device, &device.fingerprint, &native_gateway().await).await;
    let mut client = open(&bench.url).await;
    opened(&mut client).await;
    assert_eq!(visit(&mut device).await, Visit::Probe);
    assert_eq!(
        visit(&mut device).await,
        Visit::Credentials("viewer".into(), VNC_PASSWORD.into())
    );
    let (opcodes, sync) = first_frame(&mut client).await;
    assert_eq!(opcodes, ["size", "rect", "cfill", "sync"]);
    client.send(&sync).await;
    client.send("3.key,5.65293,1.1;").await;
    assert_eq!(
        timeout(Duration::from_secs(5), device.keys.recv())
            .await
            .unwrap(),
        Some((65293, true))
    );
    assert_eq!(client.close().await, Some(1000));
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../../../tests/fixtures", scripts("devices"))
)]
async fn pinned_vnc_rejects_a_changed_leaf_before_credentials(pool: PgPool) {
    let mut device = vnc_tls::device().await;
    let wrong = ["00"; 32].join(":");
    let bench = pinned(&pool, &device, &wrong, &native_gateway().await).await;
    let mut client = open(&bench.url).await;
    let error = decoded(&client.text().await);
    assert_eq!(error.args, ["Certificate changed", "771"]);
    assert_eq!(client.close_code().await, Some(4003));
    assert_eq!(visit(&mut device).await, Visit::Probe);
    ended_once(
        &ended(&pool, bench.session).await,
        "server_certificate_mismatch",
        "error",
        LOOPBACK,
    );
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../../../tests/fixtures", scripts("devices"))
)]
async fn pinned_vnc_refuses_a_gateway_without_pin_support(pool: PgPool) {
    let mut device = vnc_tls::device().await;
    let gateway = fake_guacd(Script::StallBeforeArgs { args: VNC_ARGS }).await;
    gateway.act.send(Act::Send(String::new())).unwrap();
    let bench = pinned(&pool, &device, &device.fingerprint, &gateway.address).await;
    assert_eq!(
        open_with(&bench.url, true).await.err(),
        Some(tokio_tungstenite::tungstenite::http::StatusCode::SERVICE_UNAVAILABLE)
    );
    assert_eq!(gateway.selected.await.unwrap(), "vnc");
    assert_eq!(
        timeout(Duration::from_secs(5), gateway.handshake)
            .await
            .unwrap()
            .unwrap(),
        [] as [guac_protocol::Instruction; 0]
    );
    assert_eq!(visit(&mut device).await, Visit::Probe);
    ended_once(
        &ended(&pool, bench.session).await,
        "gateway_error",
        "warning",
        LOOPBACK,
    );
}

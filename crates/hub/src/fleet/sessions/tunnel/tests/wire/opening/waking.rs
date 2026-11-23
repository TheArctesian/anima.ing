//! K4 Section 1, RA-031, R-030: a woken opening waits for the device's port up to the woken limit.

use std::time::{Duration, Instant};

use sqlx::PgPool;
use tokio::{net::TcpListener, sync::oneshot, time::timeout};
use tokio_tungstenite::tungstenite::http::StatusCode;

use super::super::{
    super::super::opening::{self, NotOpened},
    bench::{Bench, LOOPBACK, Setup, bench, now, on_tunnel},
    ended_once,
    fake_guacd::{FakeGuacd, RDP_ARGS, Script, fake_guacd},
    records::ended,
    socket::open_with,
};
use crate::fleet::{
    sessions::{
        Live, Protocol, Redeemed,
        tests::fixture::{DOCKER_HOST, answer_after, reissued_woken, waking_port},
        tickets::WOKEN_OPENING,
    },
    targets::AddressPolicy,
};

const USUAL: Duration = Duration::from_millis(200);
/// Past the usual limit and far inside the woken limit of the answering tests.
const ANSWERS_AFTER: Duration = Duration::from_millis(600);

async fn waking(pool: &PgPool, woken: Duration) -> (FakeGuacd, Bench, oneshot::Sender<()>) {
    let guacd = fake_guacd(Script::Serve {
        args: RDP_ARGS,
        with_ready: "",
    })
    .await;
    let setup = Setup::route(move |route| {
        route.timing.connect = USUAL;
        route.timing.woken_opening = woken;
        route.timing.reconnect = Duration::from_millis(50);
    });
    let bench = bench(pool, &guacd.address, setup).await;
    let (port, answer) = waking_port(None);
    move_rdp_port(pool, port).await;
    (guacd, bench, answer)
}

/// The certificate is ignored because the waking device only accepts the connection.
async fn move_rdp_port(pool: &PgPool, port: u16) {
    sqlx::query(
        "update devices.rdp_settings set port = $2, ignore_certificate = true \
         where device_id = $1::text::uuid",
    )
    .bind(DOCKER_HOST)
    .bind(i32::from(port))
    .execute(pool)
    .await
    .unwrap();
}

fn woken(bench: &Bench) -> Redeemed {
    let mut redeemed = bench.live.redeem(on_tunnel(&bench.token), now()).unwrap();
    redeemed.ticket.wake_sent = true;
    redeemed
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../../../tests/fixtures", scripts("devices"))
)]
async fn ra_031_a_woken_opening_reaches_guacd_once_the_device_answers(pool: PgPool) {
    let (_guacd, bench, answer) = waking(&pool, Duration::from_secs(10)).await;
    let redeemed = woken(&bench);
    let (began, was_begun) = oneshot::channel();
    let opening = async {
        began.send(()).unwrap();
        opening::open(&bench.route, &redeemed.ticket).await
    };
    let (opened, ()) = tokio::join!(opening, answer_after(was_begun, ANSWERS_AFTER, answer));
    assert!(
        opened.is_ok(),
        "the gateway side opens although the device answered after the usual limit"
    );
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../../../tests/fixtures", scripts("devices"))
)]
async fn ra_031_a_woken_device_that_never_answers_ends_the_opening_at_the_woken_limit(
    pool: PgPool,
) {
    const LIMIT: Duration = Duration::from_millis(600);
    let (_guacd, bench, _asleep) = waking(&pool, LIMIT).await;
    let redeemed = woken(&bench);
    let begun = Instant::now();
    let opened = timeout(
        Duration::from_secs(30),
        opening::open(&bench.route, &redeemed.ticket),
    )
    .await
    .expect("the opening ends");
    assert!(matches!(opened, Err(NotOpened::NotAwake)));
    assert!(
        begun.elapsed() >= LIMIT,
        "it waited past the usual limit, to the woken one"
    );
}

#[test]
fn ra_031_the_not_awake_message_names_the_woken_limit() {
    let seconds = format!("within {} seconds", WOKEN_OPENING.as_secs());
    for protocol in [Protocol::Rdp, Protocol::Vnc] {
        let message = NotOpened::NotAwake.message(protocol);
        assert!(message.contains(&seconds), "{message}");
    }
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../../../tests/fixtures", scripts("devices"))
)]
async fn r_030_a_woken_ticket_to_a_denied_address_is_503_at_once_and_never_reached(pool: PgPool) {
    let mut guacd = fake_guacd(Script::Serve {
        args: RDP_ARGS,
        with_ready: "",
    })
    .await;
    let woken = Duration::from_secs(10);
    let policy = AddressPolicy::new(vec!["127.0.0.0/8".parse().unwrap()]);
    let setup = Setup {
        live: Live::default()
            .reaching_loopback_for_tests()
            .denying(policy),
        ..Setup::route(move |route| {
            route.timing.woken_opening = woken;
            route.timing.reconnect = Duration::from_millis(50);
        })
    };
    let bench = bench(&pool, &guacd.address, setup).await;
    let device = TcpListener::bind((LOOPBACK, 0)).await.unwrap();
    move_rdp_port(&pool, device.local_addr().unwrap().port()).await;
    let token = reissued_woken(&bench.live, bench.session);
    let begun = Instant::now();
    let url = format!("ws://{}/tunnel?token={token}", bench.served);
    let refused = open_with(&url, true).await.err();
    assert_eq!(refused, Some(StatusCode::SERVICE_UNAVAILABLE));
    assert!(
        begun.elapsed() < woken / 2,
        "refused before the woken limit"
    );
    let ended = ended(&pool, bench.session).await;
    ended_once(&ended, "gateway_error", "warning", LOOPBACK);
    assert!(ended.events[0].3.contains("ANIMA_PROBE_DENY"), "{ended:?}");
    let reached = timeout(Duration::from_millis(50), device.accept()).await;
    assert!(reached.is_err(), "no connection reached the device");
    assert!(guacd.selected.try_recv().is_err(), "guacd is never reached");
}

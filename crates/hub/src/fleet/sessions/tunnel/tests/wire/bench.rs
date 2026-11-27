//! The session starts on a K6 device host, then the device moves to loopback, which the
//! tests' target rule lets the tunnel reach.

mod fake_rdp;

use std::{net::SocketAddr, sync::Arc, time::Duration};

use chrono::TimeDelta;
use sqlx::PgPool;
use tokio::net::TcpListener;

use super::super::super::route::{Route, Timing, router};
use crate::{
    clock::{Clock, FixedClock},
    fleet::{
        credentials::{self, PasswordKind, Secret, Vault},
        events::{Bus, ClientAddr},
        sessions::{
            DisplayRequest, Live, Presented, Protocol, StartRequest, start,
            tests::fixture::{DOCKER_HOST, add_operator, own_pool, vault},
        },
    },
    ids::SessionId,
};
use fake_rdp::{Answer, FakeRdp, fake_rdp};

pub(super) use crate::fleet::sessions::tests::fixture::{
    LOOPBACK, PASSWORD, address, docker_host, now, operator,
};

pub(super) const VNC_PASSWORD: &str = "vnc tr0ub4dor";
/// An accepted fingerprint that no fake RDP server presents.
pub(super) const FINGERPRINT: &str = "0a:1b:2c:3d:4e:5f:60:71:82:93:a4:b5:c6:d7:e8:f9:\
                                      0a:1b:2c:3d:4e:5f:60:71:82:93:a4:b5:c6:d7:e8:f9";
pub(super) const DRIVE_ROOT: &str = "/drives";

/// Waits long enough that only the test's own action ends a session.
pub(super) fn patient() -> Timing {
    Timing {
        connect: Duration::from_secs(3),
        woken_opening: Duration::from_secs(3),
        reconnect: Duration::from_millis(50),
        handshake: Duration::from_secs(3),
        gateway_silence: Duration::from_secs(60),
        client_idle: Duration::from_secs(60),
    }
}

/// Long enough for a loaded machine, far short of [`patient`]'s waits (D-10). Real sockets
/// and PostgreSQL rule out paused tokio time: auto-advance would fire the hub's waits early.
pub(super) const END_WAIT: Duration = Duration::from_secs(10);

pub(super) fn ending_within_end_wait() -> Setup {
    Setup {
        live: Live::with_tunnel_end_wait(END_WAIT).reaching_loopback_for_tests(),
        ..Setup::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Presents {
    Accepted,
    AcceptedOverTls,
    AcceptedOverNlaExt,
    Changed,
    StandardOnly,
}

pub(super) struct Setup {
    pub(super) route: Box<dyn FnOnce(&mut Route) + Send>,
    pub(super) ticket_client: ClientAddr,
    pub(super) live: Live,
    pub(super) protocol: Protocol,
    pub(super) view_only: bool,
    pub(super) presents: Presents,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            route: Box::new(|_| {}),
            ticket_client: address(LOOPBACK),
            live: Live::default().reaching_loopback_for_tests(),
            protocol: Protocol::Rdp,
            view_only: false,
            presents: Presents::Accepted,
        }
    }
}

impl Setup {
    pub(super) fn route(tune: impl FnOnce(&mut Route) + Send + 'static) -> Self {
        Self {
            route: Box::new(tune),
            ..Self::default()
        }
    }

    pub(super) fn protocol(protocol: Protocol) -> Self {
        Self {
            protocol,
            ..Self::default()
        }
    }

    pub(super) fn presents(presents: Presents) -> Self {
        Self {
            presents,
            ..Self::default()
        }
    }
}

pub(super) struct Bench {
    pub(super) live: Live,
    pub(super) route: Route,
    pub(super) session: SessionId,
    pub(super) token: String,
    pub(super) url: String,
    pub(super) served: SocketAddr,
    pub(super) rdp: FakeRdp,
    pub(super) accepted: String,
}

/// Binds both passwords to loopback, where the bench moves the device (SEC-011).
pub(super) async fn desktop_ready(pool: &PgPool, vault: &Vault, rdp_port: u16, accepted: &str) {
    add_operator(pool).await;
    sqlx::query(
        "update devices.rdp_settings set enabled = true, username = 'ops', domain = 'LAB', \
         port = $2 where device_id = $1::text::uuid",
    )
    .bind(DOCKER_HOST)
    .bind(i32::from(rdp_port))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "update devices.vnc_settings set enabled = true, username = 'viewer' \
         where device_id = $1::text::uuid",
    )
    .bind(DOCKER_HOST)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into trust.server_certificates (device_id, fingerprint_sha256, \
                first_seen_at, accepted_by) \
         select $1::text::uuid, $2, $3, id from access.operators",
    )
    .bind(DOCKER_HOST)
    .bind(accepted)
    .bind(now())
    .execute(pool)
    .await
    .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    for (kind, password) in [
        (PasswordKind::Rdp, PASSWORD),
        (PasswordKind::Vnc, VNC_PASSWORD),
    ] {
        let secret = Secret::from(password.to_owned());
        credentials::store_password(&mut conn, vault, (docker_host(), LOOPBACK), (kind, secret))
            .await
            .unwrap();
    }
}

/// Stores the same password bound to `host`, as the operator does after a host change (SEC-011).
pub(super) async fn enter_again(pool: &PgPool, bench: &Bench, kind: PasswordKind, host: &str) {
    let text = match kind {
        PasswordKind::Rdp => PASSWORD,
        PasswordKind::Vnc => VNC_PASSWORD,
    };
    let mut conn = pool.acquire().await.unwrap();
    let password = (kind, Secret::from(text.to_owned()));
    credentials::store_password(
        &mut conn,
        &bench.route.vault,
        (docker_host(), host),
        password,
    )
    .await
    .unwrap();
}

pub(super) fn request(protocol: Protocol) -> StartRequest {
    StartRequest {
        device: docker_host(),
        protocol,
        display: DisplayRequest {
            width_px: 1280,
            height_px: 800,
            dpi: 96,
            timezone: Some("Europe/Berlin".to_owned()),
        },
        view_only: false,
        wake: None,
    }
}

pub(super) async fn move_host(pool: &PgPool, host: &str) {
    sqlx::query("update devices.devices set host = $2 where id = $1::text::uuid")
        .bind(DOCKER_HOST)
        .bind(host)
        .execute(pool)
        .await
        .unwrap();
}

async fn prepare_rdp(pool: &PgPool, vault: &Vault, presents: Presents) -> (FakeRdp, String) {
    let answer = match presents {
        Presents::Accepted | Presents::Changed => Answer::Nla,
        Presents::AcceptedOverTls => Answer::Tls,
        Presents::AcceptedOverNlaExt => Answer::NlaExt,
        Presents::StandardOnly => Answer::Standard,
    };
    let rdp = fake_rdp(answer).await;
    let accepted = if matches!(
        presents,
        Presents::Accepted | Presents::AcceptedOverTls | Presents::AcceptedOverNlaExt
    ) {
        rdp.fingerprint.clone()
    } else {
        FINGERPRINT.to_owned()
    };
    desktop_ready(pool, vault, rdp.port, &accepted).await;
    (rdp, accepted)
}

pub(super) async fn bench(pool: &PgPool, guacd: &str, setup: Setup) -> Bench {
    let vault = vault();
    let (rdp, accepted) = prepare_rdp(pool, &vault, setup.presents).await;
    let clock: Arc<dyn Clock> = Arc::new(FixedClock(now()));
    let live = setup.live;
    let client = setup.ticket_client;
    let ticket = start(
        pool,
        &clock,
        &live,
        &operator(),
        client,
        StartRequest {
            view_only: setup.view_only,
            ..request(setup.protocol)
        },
    )
    .await
    .unwrap();
    move_host(pool, LOOPBACK).await;
    let mut route = Route {
        db: own_pool(pool).await,
        vault,
        bus: Bus::new(16),
        clock,
        guacd: guacd.to_owned(),
        drive_root: DRIVE_ROOT.to_owned(),
        live: live.clone(),
        timing: patient(),
    };
    (setup.route)(&mut route);
    let listener = TcpListener::bind((LOOPBACK, 0)).await.unwrap();
    let served = listener.local_addr().unwrap();
    let app = router(route.clone()).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Bench {
        live,
        route,
        session: ticket.session,
        token: ticket.token().to_owned(),
        url: format!("ws://{served}/tunnel?token={}", ticket.token()),
        served,
        rdp,
        accepted,
    }
}

/// As guacamole-common-js presents the token.
pub(super) fn on_tunnel(token: &str) -> Presented<'_> {
    Presented {
        path: "/tunnel",
        ..Presented::terminal(token, address(LOOPBACK))
    }
}

pub(super) fn clock_at(seconds: i64) -> Arc<dyn Clock> {
    Arc::new(FixedClock(now() + TimeDelta::seconds(seconds)))
}

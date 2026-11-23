//! The SSH session starts while the device's host is the K6 address `10.10.0.21`; then the
//! device moves to the test SSH server on loopback, which the tests' deny list allows.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use serde_json::Value;
use sqlx::PgPool;
use tokio::net::TcpListener;

use super::{
    server::{Server, serve},
    socket::{self, Socket},
};
use crate::{
    clock::{Clock, FixedClock},
    fleet::{
        events::Bus,
        sessions::{
            Actor, Live, Presented, Redeemed,
            files::{
                opening::open_files,
                route::{Files, Timing, router},
            },
            start,
            tests::fixture::{
                LOOPBACK, address, move_device, now, operator, own_pool, prepare, request, vault,
            },
        },
        targets::AddressPolicy,
    },
    ids::SessionId,
};

const SEED: u8 = 11;

pub(super) fn patient() -> Timing {
    Timing {
        opening: Duration::from_secs(5),
        hub_silence: Duration::from_secs(60),
        client_idle: Duration::from_secs(60),
        device: Duration::from_secs(5),
        keep_alive: Duration::from_secs(60),
    }
}

pub(super) struct Bench {
    pub(super) db: PgPool,
    pub(super) live: Live,
    pub(super) server: Server,
    pub(super) session: SessionId,
    /// Dropping it ends the session's tunnel.
    pub(super) terminal: Option<Redeemed>,
    pub(super) served: SocketAddr,
}

pub(super) async fn bench_with(pool: &PgPool, tune: impl FnOnce(&mut Files)) -> Bench {
    let vault = vault();
    let server = serve(SEED).await;
    prepare(pool, &vault, &server.key).await;
    let clock: Arc<dyn Clock> = Arc::new(FixedClock(now()));
    let live = Live::with_tunnel_end_wait(Duration::from_secs(10));
    let client = address(LOOPBACK);
    let ticket = start(pool, &clock, &live, &operator(), client, request())
        .await
        .unwrap();
    let presented = Presented::terminal(ticket.token(), client);
    let terminal = live.redeem(presented, now()).unwrap();
    move_device(pool, LOOPBACK, server.port).await;
    let mut files = Files {
        db: own_pool(pool).await,
        vault,
        bus: Bus::new(64),
        clock,
        live: live.clone(),
        policy: AddressPolicy::default().allowing_loopback(),
        timing: patient(),
        max_entries: 10_000,
    };
    tune(&mut files);
    let listener = TcpListener::bind((LOOPBACK, 0)).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = router(files).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Bench {
        served: bound,
        db: pool.clone(),
        live,
        server,
        session: ticket.session,
        terminal: Some(terminal),
    }
}

pub(super) fn by(client: &str) -> Actor {
    Actor {
        operator: operator(),
        client: address(client),
        at: now(),
    }
}

pub(super) async fn bench(pool: &PgPool) -> Bench {
    bench_with(pool, |_| {}).await
}

impl Bench {
    pub(super) async fn token(&self) -> String {
        let ticket = open_files(&self.db, &self.live, &by(LOOPBACK), self.session)
            .await
            .unwrap();
        assert_eq!(ticket.tunnel_path, "/files");
        ticket.token().to_owned()
    }

    pub(super) fn url(&self, token: &str) -> String {
        format!("ws://{}/files?token={token}", self.served)
    }

    pub(super) async fn open(&self) -> Socket {
        let token = self.token().await;
        socket::open(&self.url(&token)).await
    }

    pub(super) async fn flags(&self, upload: bool, download: bool) {
        sqlx::query(
            "update devices.devices set transfer_upload = $1, transfer_download = $2 \
             where name = 'docker-host'",
        )
        .bind(upload)
        .bind(download)
        .execute(&self.db)
        .await
        .unwrap();
    }

    pub(super) async fn audits(&self) -> Vec<(String, String, String, Value)> {
        sqlx::query_as(
            "select action, operator_name, host(client_addr), change from events.audit_log \
             where target_type = 'file' order by at, id",
        )
        .fetch_all(&self.db)
        .await
        .unwrap()
    }

    pub(super) async fn events(&self) -> Vec<(String, String, Option<String>, String)> {
        sqlx::query_as(
            "select severity, source, device_name, message from events.event_log \
             order by at, id",
        )
        .fetch_all(&self.db)
        .await
        .unwrap()
    }

    pub(super) async fn contexts(&self) -> Vec<Value> {
        sqlx::query_scalar(
            "select to_jsonb(c) || jsonb_build_object('eventStored', exists( \
             select 1 from events.event_log e where e.id = c.event_id)) \
             from locations.transfer_context c order by occurred_at, event_id",
        )
        .fetch_all(&self.db)
        .await
        .unwrap()
    }

    pub(super) async fn sited(&self) -> (String, String, String) {
        sqlx::query_as(
            "with made as (insert into places.place \
             (id, name, latitude, longitude, radius_m, category, icon, created_at, updated_at) \
             values (gen_random_uuid(), 'Office', 52.0, 4.0, 100, 'work', 'desk', now(), now()) \
             returning id), linked as (insert into places.site_place (site_id, place_id) \
             select d.site_id, made.id from devices.devices d, made where d.name = 'docker-host' \
             returning site_id, place_id) \
             select d.id::text, l.site_id::text, l.place_id::text \
             from devices.devices d, linked l where d.name = 'docker-host'",
        )
        .fetch_one(&self.db)
        .await
        .unwrap()
    }
}

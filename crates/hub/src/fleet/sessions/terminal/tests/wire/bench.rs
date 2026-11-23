//! The session starts on a K6 device host, then the device moves to the fake SSH server on
//! loopback, which the tests' target rule lets the hub reach (R-032).

use std::{net::SocketAddr, sync::Arc, time::Duration};

use sqlx::PgPool;
use tokio::net::TcpListener;

use super::{
    super::super::route::{Terminal, Timing, router},
    fake_ssh::{FakeSsh, Script, fake_ssh},
};
use crate::{
    clock::{Clock, FixedClock},
    fleet::{
        events::{Bus, ClientAddr},
        sessions::{
            Live, start,
            tests::fixture::{
                LOOPBACK, address, move_device, now, operator, own_pool, prepare, request, vault,
            },
        },
        trust::PublicKey,
    },
    ids::SessionId,
};

const HOST_KEY_SEED: u8 = 5;

/// Waits long enough that only the test's own action ends a session.
pub(super) fn patient() -> Timing {
    Timing {
        opening: Duration::from_secs(5),
        woken_opening: Duration::from_secs(5),
        reconnect: Duration::from_millis(50),
        upgrade: Duration::from_secs(5),
        first_size: Duration::from_secs(60),
        hub_silence: Duration::from_secs(60),
        client_idle: Duration::from_secs(60),
        request: Duration::from_secs(60),
    }
}

/// Far short of [`patient`]'s waits, so only the end signal ends a session within it (D-10).
pub(super) const END_WAIT: Duration = Duration::from_secs(10);

pub(super) struct Setup {
    pub(super) terminal: Box<dyn FnOnce(&mut Terminal) + Send>,
    pub(super) ticket_client: ClientAddr,
    pub(super) live: Live,
    pub(super) accepted: Option<PublicKey>,
    pub(super) script: Script,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            terminal: Box::new(|_| {}),
            ticket_client: address(LOOPBACK),
            live: Live::with_tunnel_end_wait(END_WAIT).reaching_loopback_for_tests(),
            accepted: None,
            script: Script::default(),
        }
    }
}

impl Setup {
    pub(super) fn terminal(tune: impl FnOnce(&mut Terminal) + Send + 'static) -> Self {
        Self {
            terminal: Box::new(tune),
            ..Self::default()
        }
    }

    pub(super) fn script(script: Script) -> Self {
        Self {
            script,
            ..Self::default()
        }
    }
}

pub(super) struct Bench {
    pub(super) terminal: Terminal,
    pub(super) live: Live,
    pub(super) session: SessionId,
    pub(super) token: String,
    pub(super) url: String,
    pub(super) served: SocketAddr,
    pub(super) ssh: FakeSsh,
}

pub(super) async fn bench(pool: &PgPool, setup: Setup) -> Bench {
    let vault = vault();
    let ssh = fake_ssh(HOST_KEY_SEED, setup.script).await;
    prepare(pool, &vault, setup.accepted.as_ref().unwrap_or(&ssh.key)).await;
    let clock: Arc<dyn Clock> = Arc::new(FixedClock(now()));
    let live = setup.live;
    let ticket = start(
        pool,
        &clock,
        &live,
        &operator(),
        setup.ticket_client,
        request(),
    )
    .await
    .unwrap();
    move_device(pool, LOOPBACK, ssh.port).await;
    let mut terminal = Terminal {
        db: own_pool(pool).await,
        vault,
        bus: Bus::new(16),
        clock,
        live: live.clone(),
        timing: patient(),
    };
    (setup.terminal)(&mut terminal);
    let listener = TcpListener::bind((LOOPBACK, 0)).await.unwrap();
    let served = listener.local_addr().unwrap();
    let app = router(terminal.clone()).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Bench {
        terminal,
        live,
        session: ticket.session,
        token: ticket.token().to_owned(),
        url: format!("ws://{served}/terminal?token={}", ticket.token()),
        served,
        ssh,
    }
}

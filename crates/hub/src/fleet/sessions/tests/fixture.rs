use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::Value;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::sync::oneshot;

use crate::{
    fleet::{
        events::{ClientAddr, Operator},
        sessions::{
            DisplayRequest, Live, PendingTicket, Protocol, StartRequest,
            tickets::{TOKEN_BYTES, TicketToken},
        },
    },
    ids::{OperatorId, SessionId},
};

pub(in crate::fleet::sessions) use crate::fleet::ssh::tests::fixture::{
    DOCKER_HOST, LOOPBACK, OPERATOR, PASSWORD, add_operator, docker_host, move_device, now,
    prepare, vault,
};

pub(in crate::fleet::sessions) fn operator() -> Operator {
    Operator {
        id: OPERATOR.parse::<OperatorId>().unwrap(),
        name: "operator".to_owned(),
        actor: crate::fleet::events::Actor::for_tests(),
    }
}

pub(in crate::fleet::sessions) fn address(text: &str) -> ClientAddr {
    ClientAddr(text.parse().unwrap())
}

/// sqlx test pools share 20 connections across all running tests; the session task writes
/// while the test reads, and two connections per test starve that share under load.
pub(in crate::fleet::sessions) async fn own_pool(pool: &PgPool) -> PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap()
}

/// The device answers when the test says, not after a time, so a loaded machine cannot move
/// the answer across an opening limit.
pub(in crate::fleet::sessions) fn waking_port(forward: Option<u16>) -> (u16, oneshot::Sender<()>) {
    let reserved = std::net::TcpListener::bind((LOOPBACK, 0)).unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);
    let (answer, answered) = oneshot::channel();
    tokio::spawn(async move {
        if answered.await.is_err() {
            return;
        }
        let listener = tokio::net::TcpListener::bind((LOOPBACK, port))
            .await
            .unwrap();
        while let Ok((mut client, _)) = listener.accept().await {
            let Some(forward) = forward else { continue };
            tokio::spawn(async move {
                let mut device = tokio::net::TcpStream::connect((LOOPBACK, forward)).await?;
                tokio::io::copy_bidirectional(&mut client, &mut device).await
            });
        }
    });
    (port, answer)
}

/// `began` fires in the poll that starts the opening's limit, so the device answers at least
/// `pause` after the limit started, however late the machine schedules either side.
pub(in crate::fleet::sessions) async fn answer_after(
    began: oneshot::Receiver<()>,
    pause: Duration,
    answer: oneshot::Sender<()>,
) {
    began.await.unwrap();
    tokio::time::sleep(pause).await;
    answer.send(()).unwrap();
}

pub(in crate::fleet::sessions) fn reissued_woken(live: &Live, session: SessionId) -> String {
    let ticket = live.withdraw(session).unwrap();
    let token = TicketToken::from_random([3; TOKEN_BYTES]);
    let woken = PendingTicket {
        wake_sent: true,
        ..ticket
    };
    live.issue(token.hash(), woken).unwrap();
    token.expose().to_owned()
}

pub(in crate::fleet::sessions) fn request() -> StartRequest {
    StartRequest {
        device: docker_host(),
        protocol: Protocol::Ssh,
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

#[derive(Debug)]
pub(in crate::fleet::sessions) struct Ended {
    pub(in crate::fleet::sessions) reason: String,
    /// `(operator name, client address, change)` of each `session.end` entry.
    pub(in crate::fleet::sessions) audits: Vec<(String, String, Value)>,
    /// `(severity, source, device name, message)` of each event.
    pub(in crate::fleet::sessions) events: Vec<(String, String, Option<String>, String)>,
}

pub(in crate::fleet::sessions) async fn ended(pool: &PgPool, session: SessionId) -> Ended {
    for _ in 0..250 {
        let reason: Option<String> =
            sqlx::query_scalar("select end_reason from sessions.sessions where id = $1")
                .bind(session)
                .fetch_one(pool)
                .await
                .unwrap();
        if let Some(reason) = reason {
            // The audit entry and event commit with the row.
            return Ended {
                reason,
                audits: end_audits(pool, session).await,
                events: events(pool).await,
            };
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the session did not end");
}

async fn end_audits(pool: &PgPool, session: SessionId) -> Vec<(String, String, Value)> {
    sqlx::query_as(
        "select operator_name, host(client_addr), change from events.audit_log \
         where action = 'end' and target_type = 'session' and target_id = $1",
    )
    .bind(session.to_string())
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn events(pool: &PgPool) -> Vec<(String, String, Option<String>, String)> {
    sqlx::query_as("select severity, source, device_name, message from events.event_log")
        .fetch_all(pool)
        .await
        .unwrap()
}

pub(in crate::fleet::sessions) fn ended_once(
    ended: &Ended,
    reason: &str,
    severity: &str,
    client: &str,
) {
    assert_eq!(ended.reason, reason);
    let audit = (
        "operator".to_owned(),
        client.to_owned(),
        serde_json::json!({ "reason": reason }),
    );
    assert_eq!(ended.audits, [audit], "one session.end entry");
    assert_eq!(ended.events.len(), 1, "one event: {:?}", ended.events);
    let (got_severity, source, device, _) = &ended.events[0];
    assert_eq!(
        (got_severity.as_str(), source.as_str()),
        (severity, "session")
    );
    assert_eq!(device.as_deref(), Some("docker-host"));
}

/// The actor kinds of the session's `session.end` entries and of every event (LOG-012, LOG-013).
pub(in crate::fleet::sessions) async fn end_kinds(
    pool: &PgPool,
    session: SessionId,
) -> (Vec<(String, Option<String>)>, Vec<Option<String>>) {
    let audits = sqlx::query_as(
        "select operator_name, actor_kind from events.audit_log \
         where action = 'end' and target_type = 'session' and target_id = $1",
    )
    .bind(session.to_string())
    .fetch_all(pool)
    .await
    .unwrap();
    let events = sqlx::query_scalar("select actor_kind from events.event_log")
        .fetch_all(pool)
        .await
        .unwrap();
    (audits, events)
}

pub(in crate::fleet::sessions) async fn stored_text(pool: &PgPool) -> String {
    let rows: Vec<String> = sqlx::query_scalar(
        "select row_to_json(s)::text from sessions.sessions s \
         union all select row_to_json(a)::text from events.audit_log a \
         union all select row_to_json(e)::text from events.event_log e",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    rows.join("\n")
}

#[derive(Clone, Default)]
pub(in crate::fleet::sessions) struct Capture(pub(in crate::fleet::sessions) Arc<Mutex<Vec<u8>>>);

impl io::Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

//! K4 Sections 4 and 8.5, MV-003.

use std::{sync::Arc, time::Duration};

use sqlx::PgPool;
use tokio::{task::JoinHandle, time::sleep};

use super::{bench::Setup, fake_ssh::FakeSsh, session};
use crate::fleet::sessions::tests::fixture::{LOOPBACK, ended, ended_once};

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../../tests/fixtures", scripts("devices"))
)]
async fn k4_8_5_a_send_that_stalls_is_timeout_4002(pool: PgPool) {
    let idle = Duration::from_secs(1);
    let setup = Setup::terminal(move |terminal| terminal.timing.client_idle = idle);
    let (bench, mut client) = session(&pool, setup).await;
    let flood = flood_unread(&Arc::new(bench.ssh));
    // The client drains only after the send stalled, while the hub's `end` and close are on
    // their way.
    tokio::time::sleep(idle + Duration::from_millis(500)).await;
    assert_eq!(client.ended().await, ("TIMEOUT".to_owned(), Some(4002)));
    let ended = ended(&pool, bench.session).await;
    ended_once(&ended, "timeout", "info", LOOPBACK);
    assert_eq!(
        ended.events[0].3,
        "The client stopped reading what the hub sent, so the hub closed the SSH session."
    );
    flood.abort();
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../../tests/fixtures", scripts("devices"))
)]
async fn k4_8_5_mv_003_a_send_to_a_client_that_reset_the_connection_is_operator(pool: PgPool) {
    // Dropping a client with data unread resets the connection: the client left.
    let setup = Setup::terminal(|terminal| terminal.timing.client_idle = Duration::from_secs(30));
    let (bench, client) = session(&pool, setup).await;
    let flood = flood_unread(&Arc::new(bench.ssh));
    sleep(Duration::from_millis(700)).await;
    drop(client);
    let ended = ended(&pool, bench.session).await;
    ended_once(&ended, "operator", "info", LOOPBACK);
    flood.abort();
}

fn flood_unread(ssh: &Arc<FakeSsh>) -> JoinHandle<()> {
    let writer = Arc::clone(ssh);
    tokio::spawn(async move {
        let chunk = vec![b'x'; 32 * 1024];
        for _ in 0..1_024 {
            if writer.try_say(&chunk).await.is_err() {
                return;
            }
        }
    })
}

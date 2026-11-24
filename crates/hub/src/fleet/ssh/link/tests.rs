//! In-crate tests, so the loopback policy stays crate-private.

mod command;
mod opening;
mod running;
pub(crate) mod server;

use std::time::Duration;

use sqlx::PgPool;

use server::{Answer, Server, serve};

use super::{Error, SshLink, link};
use crate::fleet::{
    credentials::Vault,
    devices::{self, Device},
    ssh::tests::fixture::{LOOPBACK, PASSWORD, docker_host, move_device, now, prepare, vault},
    targets::AddressPolicy,
};

pub(crate) const SEED: u8 = 9;
const LIMIT: Duration = Duration::from_secs(5);

pub(crate) struct Bench {
    pub(crate) db: PgPool,
    pub(crate) vault: Vault,
    pub(crate) device: Device,
    pub(crate) server: Server,
}

/// The only policy that reaches the loopback test server: no release build has it.
pub(crate) fn reaching() -> AddressPolicy {
    AddressPolicy::default().allowing_loopback()
}

pub(crate) async fn bench(pool: &PgPool, answer: Answer) -> Bench {
    let server = serve(SEED, answer).await;
    bench_accepting(pool, server, server::host_key(SEED)).await
}

pub(crate) async fn bench_accepting(
    pool: &PgPool,
    server: Server,
    accepted: crate::fleet::trust::PublicKey,
) -> Bench {
    let vault = vault();
    prepare(pool, &vault, &accepted).await;
    move_device(pool, LOOPBACK, server.port).await;
    let device = devices::get(pool, docker_host()).await.unwrap();
    Bench {
        db: pool.clone(),
        vault,
        device,
        server,
    }
}

impl Bench {
    async fn link_with(&self, policy: &AddressPolicy) -> Result<SshLink, Error> {
        link(&self.db, &self.vault, policy, &self.device, now()).await
    }

    async fn opened(&self) -> SshLink {
        let Ok(link) = self.link_with(&reaching()).await else {
            panic!("the link opens");
        };
        link
    }
}

/// The error's message and debug form hold no address, host, port, device text or secret (LOG-007).
fn assert_quiet(error: &Error, port: u16) {
    for text in [error.to_string(), format!("{error:?}")] {
        for secret in [
            LOOPBACK,
            "docker-host",
            "invalid",
            PASSWORD,
            &port.to_string(),
        ] {
            assert!(!text.contains(secret), "{text:?} holds {secret:?}");
        }
    }
}

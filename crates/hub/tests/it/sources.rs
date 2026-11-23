//! The sources engine against a loopback GitHub (K-E1; SRC-003 to SRC-012, PRJ-018, EXT-002).

mod changes;
mod connect;
mod controls;
mod conversion;
mod failures;
mod installations;
mod intake;
mod leaks;
mod live;
mod registration;
mod runs;
pub mod stub;
mod transport;

use std::sync::{Arc, LazyLock};

use anima_hub::{
    clock::{Clock, FixedClock},
    engine::sources::{self, Outcome, Reach, Registry, Scheduling, Transport, Trigger},
    fleet::{events::Bus, targets::AddressPolicy},
    ids::SourceAccountId,
    schema::Dependencies,
};
use chrono::{DateTime, Utc};
use reqwest::Url;
use rsa::{
    RsaPrivateKey,
    pkcs1::{EncodeRsaPrivateKey as _, LineEnding},
    rand_core::OsRng,
};
use serde_json::{Value, json};
use source_exchange::Declaration;
use sqlx::PgPool;

use self::{intake::StubIntake, stub::Stub};
use crate::hub::{Hub, dependencies, set_operator, start, test_vault};

/// A throwaway 2048-bit App key made in this test process; never a key file.
pub static TEST_KEY: LazyLock<String> = LazyLock::new(|| {
    let key = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
    key.to_pkcs1_pem(LineEnding::LF).unwrap().to_string()
});

pub const CONNECT: &str = "mutation($input: GithubAppInput!) { connectGithubApp(input: $input) { \
    id sourceKey label instanceKey credentialStored health pausedBy syncing failureCount } }";

pub struct Kit {
    pub pool: PgPool,
    pub hub: Hub,
    pub token: String,
    pub stub: Stub,
    pub intake: StubIntake,
    pub reach: Reach,
}

impl Kit {
    pub fn at(&self, now: DateTime<Utc>) -> Scheduling {
        let clock: Arc<dyn Clock> = Arc::new(FixedClock(now));
        Scheduling::new(
            (self.pool.clone(), Bus::new(16), clock),
            test_vault(),
            self.reach.clone(),
        )
    }

    pub async fn run(&self, id: SourceAccountId) -> Option<Outcome> {
        sources::run(&self.at(start()), id, Trigger::Manual)
            .await
            .unwrap()
    }

    pub async fn connect(&self) -> (Value, SourceAccountId) {
        let response = self
            .hub
            .post(Some(&self.token), CONNECT, input(&TEST_KEY))
            .await;
        let id = response["data"]["connectGithubApp"]["id"]
            .as_str()
            .unwrap_or_else(|| panic!("connect failed: {response}"))
            .parse()
            .unwrap();
        (response, id)
    }
}

pub fn input(key: &str) -> Value {
    json!({ "input": { "label": "Mine", "appId": "4242", "installationId": "77", "privateKey": key } })
}

pub async fn kit(pool: &PgPool) -> Kit {
    let stub = Stub::start(pool).await;
    let transport = Transport::to_stub(
        stub.base.clone(),
        AddressPolicy::default().allowing_loopback(),
    )
    .unwrap();
    kit_with(pool, stub, transport).await
}

pub async fn kit_with(pool: &PgPool, stub: Stub, transport: Transport) -> Kit {
    kit_of(pool, (stub, transport), provider_github::DECLARATION).await
}

pub async fn kit_of(
    pool: &PgPool,
    (stub, transport): (Stub, Transport),
    declaration: Declaration,
) -> Kit {
    intake::make_table(pool).await;
    set_operator(pool).await;
    let intake = StubIntake::default();
    let registry = Registry::new(vec![declaration], vec![Arc::new(intake.clone())]);
    let reach = Reach::new(registry, transport);
    let clock: Arc<dyn Clock> = Arc::new(FixedClock(start()));
    let parts = Dependencies {
        sources: reach.clone(),
        ..dependencies(pool, clock, Bus::new(16))
    };
    let hub = Hub::serving(pool, parts);
    let token = hub.token().await;
    Kit {
        pool: pool.clone(),
        hub,
        token,
        stub,
        intake,
        reach,
    }
}

pub fn unreachable() -> Transport {
    let closed = Url::parse("http://127.0.0.1:9").unwrap();
    Transport::to_stub(closed, AddressPolicy::default().allowing_loopback()).unwrap()
}

pub async fn set_projects(pool: &PgPool, on: bool) {
    sqlx::query(
        "INSERT INTO extensions.switches (key, is_on, changed_at, changed_by) \
         VALUES ('projects', $1, now(), 'test') ON CONFLICT (key) DO UPDATE SET is_on = $1",
    )
    .bind(on)
    .execute(pool)
    .await
    .unwrap();
}

pub async fn items(pool: &PgPool, account: SourceAccountId) -> Vec<(String, String, i32)> {
    sqlx::query_as(
        "SELECT id, value, writes FROM public.stub_records WHERE account_id = $1 ORDER BY id",
    )
    .bind(account)
    .fetch_all(pool)
    .await
    .unwrap()
}

pub async fn slice(pool: &PgPool, account: SourceAccountId) -> Value {
    sqlx::query_scalar(
        "SELECT coalesce(sync_state -> 'test.items', 'null') FROM sources.accounts WHERE id = $1",
    )
    .bind(account)
    .fetch_one(pool)
    .await
    .unwrap()
}

pub type RunRow = (String, Option<String>, i32, i32, i32, i32, Option<String>);

pub async fn last_run(pool: &PgPool, account: SourceAccountId) -> RunRow {
    sqlx::query_as(
        "SELECT trigger, outcome, inserted, changed, unchanged, removed, error FROM sources.sync_runs \
         WHERE account_id = $1 ORDER BY started_at DESC, id DESC LIMIT 1",
    )
    .bind(account)
    .fetch_one(pool)
    .await
    .unwrap()
}

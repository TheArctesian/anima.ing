//! The installation of a computer with no application through the hub and a fake link
//! (hosted-mode.md HM-T33, R-240 c): a signed-in operator makes an invite and a claim, and only
//! the computer that joined with that invite trades the claim, once, through its link, for a
//! daemon enrolment code of its own device, which the hub adds in the site `Link`. No invite,
//! claim or code reaches an entry or a log line (LOG-007).

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anima_hub::fleet::events::process_log;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::control::{FakeLink, INVITE, KEY, code, hub_over, ok, written};
use crate::{hub::Hub, logs::Capture};

const COMPUTER: &str = "[fd41:4e49:4d41::7]";
const CREATE: &str = "mutation($n: String!) { \
    createComputerInstall(name: $n) { invite claim expiresAt } }";
const CLAIM: &str = "mutation($c: String!, $os: OperatingSystem!) { \
    claimComputerEnrolment(claim: $c, os: $os) { code expiresAt } }";
const DEVICES: &str =
    "{ devices(first: 50) { edges { node { name host kind os site { name subnet } } } } }";

/// A link that lists the computer `build-1` at `COMPUTER` once an invite for it was made.
fn computer_link() -> FakeLink {
    let joined = Arc::new(AtomicBool::new(false));
    FakeLink::start(move |request| match request["verb"].as_str() {
        Some("invites.make") => {
            joined.store(true, Ordering::SeqCst);
            ok(&json!({ "invite": INVITE, "expires": "2030-05-06T12:00:00Z" }))
        }
        _ if joined.load(Ordering::SeqCst) => ok(&json!([{
            "key": KEY, "fingerprint": "1a2b3c4d", "name": "build-1",
            "address": "fd41:4e49:4d41::7", "expires": "2030-08-03T12:00:00Z",
            "streams": 1, "path": "direct",
        }])),
        _ => ok(&json!([])),
    })
}

async fn install(hub: &Hub, token: &str) -> Value {
    let made = hub
        .post(Some(token), CREATE, json!({ "n": "build-1" }))
        .await;
    assert!(made["errors"].is_null(), "{made}");
    made["data"]["createComputerInstall"].clone()
}

async fn claim(computer: &Hub, claim: &Value) -> Value {
    computer
        .post(None, CLAIM, json!({ "c": claim, "os": "LINUX" }))
        .await
}

async fn devices(hub: &Hub, token: &str) -> Vec<Value> {
    let listed = hub.post(Some(token), DEVICES, json!({})).await;
    let edges = listed["data"]["devices"]["edges"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"));
    edges.iter().map(|edge| edge["node"].clone()).collect()
}

#[sqlx::test(migrations = "./migrations")]
async fn hm_t33_the_computer_that_joined_trades_the_claim_for_a_code_of_its_new_device(
    pool: PgPool,
) {
    let capture = Capture::default();
    let writer = capture.clone();
    let subscriber = process_log::subscriber("debug", move || writer.clone()).unwrap();
    let _default = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();
    let link = computer_link();
    let (hub, token) = hub_over(&pool, &link).await;
    let computer = hub.from(COMPUTER);

    let made = install(&hub, &token).await;
    assert_eq!(made["invite"], INVITE);
    let secret = made["claim"].as_str().unwrap().to_owned();
    assert_eq!(secret.len(), 43, "256 bits in base64url");

    let enrolled = claim(&computer, &made["claim"]).await;
    assert!(enrolled["errors"].is_null(), "{enrolled}");
    let enrolment = enrolled["data"]["claimComputerEnrolment"]["code"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(enrolment.starts_with("anima_enrol_"), "{enrolment}");
    assert_eq!(
        devices(&hub, &token).await,
        [json!({
            "name": "build-1", "host": "fd41:4e49:4d41::7", "kind": "server", "os": "LINUX",
            "site": { "name": "Link", "subnet": "fd41:4e49:4d41::/48" },
        })]
    );

    let again = claim(&computer, &made["claim"]).await;
    assert_eq!(
        code(&again),
        "UNAUTHENTICATED",
        "a claim works once: {again}"
    );
    let stored = written(&pool).await;
    assert!(
        stored.contains("Made a daemon enrolment code for build-1."),
        "{stored}"
    );
    for text in [stored, capture.text()] {
        for secret in [INVITE, secret.as_str(), enrolment.as_str()] {
            assert!(!text.contains(secret), "{text}");
        }
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn hm_t33_a_claim_from_another_computer_or_off_the_link_is_refused_and_spent(pool: PgPool) {
    let link = computer_link();
    let (hub, token) = hub_over(&pool, &link).await;
    let made = install(&hub, &token).await;

    // From this computer: not the link. The claim is not spent by a sender off the link.
    assert_eq!(code(&claim(&hub, &made["claim"]).await), "UNAUTHENTICATED");
    // From a different computer of the link: refused, and the claim is spent.
    let other = hub.from("[fd41:4e49:4d41::9]");
    assert_eq!(
        code(&claim(&other, &made["claim"]).await),
        "UNAUTHENTICATED"
    );
    let computer = hub.from(COMPUTER);
    assert_eq!(
        code(&claim(&computer, &made["claim"]).await),
        "UNAUTHENTICATED"
    );
    assert_eq!(devices(&hub, &token).await, Vec::<Value>::new());
}

#[sqlx::test(migrations = "./migrations")]
async fn hm_t33_a_second_install_of_a_computer_on_the_link_gets_a_claim_and_keeps_its_device(
    pool: PgPool,
) {
    let link = computer_link();
    let (hub, token) = hub_over(&pool, &link).await;
    let computer = hub.from(COMPUTER);
    let first = install(&hub, &token).await;
    let enrolled = claim(&computer, &first["claim"]).await;
    assert!(enrolled["errors"].is_null(), "{enrolled}");

    let second = install(&hub, &token).await;
    assert!(second["invite"].is_null(), "{second}");
    let enrolled = claim(&computer, &second["claim"]).await;
    assert!(enrolled["errors"].is_null(), "{enrolled}");
    assert_eq!(devices(&hub, &token).await.len(), 1);
    let invites = link
        .changes()
        .into_iter()
        .filter(|change| change["verb"] == "invites.make")
        .count();
    assert_eq!(invites, 1, "one invite only");
}

#[sqlx::test(migrations = "./migrations")]
async fn hm_t33_the_install_needs_a_signed_in_operator(pool: PgPool) {
    let link = computer_link();
    let (hub, _) = hub_over(&pool, &link).await;
    let response = hub.post(None, CREATE, json!({ "n": "build-1" })).await;
    assert_eq!(code(&response), "UNAUTHENTICATED", "{response}");
    assert_eq!(link.changes(), Vec::<Value>::new());
}

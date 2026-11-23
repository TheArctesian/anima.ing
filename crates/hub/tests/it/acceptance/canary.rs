//! Criterion 22 in one log (G-13, G-14): the whole hub takes every merged Phase 3 secret, and
//! its process log at TRACE for every target and every thread holds none of them (LOG-007).
//! Notification egress and session tokens need a network, SSH or guacd; their own LOG-007 tests cover them.

mod capture;
mod channels;
mod credentials;
mod daemons;
mod edge;
mod forms;
mod keys;
mod operator;
mod vpn;

use chrono::{DateTime, DurationRound as _, TimeDelta, Utc};
use sqlx::PgPool;

use self::{edge::WholeHub, forms::Leak};

#[derive(Default)]
pub struct Secrets(Vec<Secret>);

pub struct Secret {
    kind: &'static str,
    text: String,
    random: String,
    digits: bool,
}

const PREFIXES: [&str; 3] = ["anima_daemon_", "anima_enrol_", "anima_"];

impl Secrets {
    pub fn plant(&mut self, kind: &'static str) -> String {
        self.plant_with(kind, |random| format!("canary-{kind}-{random}"))
    }

    /// Plants a secret that `shape` makes from random hex, for a secret whose form the hub checks.
    pub fn plant_with(&mut self, kind: &'static str, shape: impl Fn(&str) -> String) -> String {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes).unwrap();
        let random = bytes.into_iter().fold(String::new(), forms::hex);
        let text = shape(&random);
        let kept = (0..=random.len())
            .rev()
            .find(|&n| text.contains(&random[..n]));
        let random = random[..kept.unwrap_or(0)].to_owned();
        self.0.push(Secret {
            kind,
            text: text.clone(),
            random,
            digits: false,
        });
        text
    }

    pub fn keep(&mut self, kind: &'static str, text: &str) {
        assert!(!text.is_empty(), "no {kind}");
        let random = PREFIXES.iter().find_map(|prefix| text.strip_prefix(prefix));
        self.0.push(Secret {
            kind,
            text: text.to_owned(),
            random: random.unwrap_or(text).to_owned(),
            digits: false,
        });
    }

    pub fn keep_code(&mut self, kind: &'static str, code: &str) {
        assert!(code.bytes().all(|b| b.is_ascii_digit()), "a code of digits");
        self.0.push(Secret {
            kind,
            text: code.to_owned(),
            random: String::new(),
            digits: true,
        });
    }

    fn kinds(&self) -> Vec<&'static str> {
        self.0.iter().map(|secret| secret.kind).collect()
    }
}

/// The database clock to 10 s: deliveries and pushed points go to partitions near it.
async fn database_now(pool: &PgPool) -> DateTime<Utc> {
    let now: DateTime<Utc> = sqlx::query_scalar("select pg_catalog.now()")
        .fetch_one(pool)
        .await
        .unwrap();
    now.duration_trunc(TimeDelta::seconds(10)).unwrap()
}

const EXPECTED: [&str; 35] = [
    "password",
    "wrong-password",
    "url-password",
    "browser-cookie",
    "totp-secret",
    "totp-secret-hex",
    "totp-code",
    "login-token",
    "api-key",
    "unknown-api-key",
    "daemon-code",
    "daemon-token",
    "daemon-next-token",
    "webhook-path",
    "webhook-query",
    "webhook-header",
    "webhook-new-header",
    "refused-webhook-path",
    "ntfy-topic",
    "ntfy-token",
    "smtp-password",
    "smtp-recipient",
    "apprise-url",
    "apprise-key",
    "ssh-password",
    "ssh-key",
    "ssh-passphrase",
    "unreadable-ssh",
    "tested-ssh-password",
    "rdp-password",
    "vnc-password",
    "vpn-invite-key",
    "vpn-router-key",
    "vpn-unreadable-reply",
    "vpn-auth-id",
];

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn log_007_no_phase_3_secret_reaches_the_process_log(pool: PgPool) {
    let watch = capture::everywhere();

    let now = database_now(&pool).await;
    let mut secrets = Secrets::default();
    let broker = vpn::FakeBroker::start(&mut secrets);
    let feed = anima_hub::fleet::vpn::Feed::new();
    let hub = WholeHub::new(&pool, now, (feed.clone(), broker.broker())).await;
    let token = operator::sign_in(&hub, &pool, now, &mut secrets).await;
    keys::api_key(&hub, &token, now, &mut secrets).await;
    daemons::enrol(&hub, &token, &mut secrets).await;
    channels::send_tests(&hub, &token, &mut secrets).await;
    credentials::set(&hub, &token, &mut secrets).await;
    vpn::change(&hub, &token, (&pool, &feed, &broker), now).await;
    // A line after the requests proves the log reached the buffer: an empty capture is no pass.
    tracing::error!("capture control line");
    let blocking = tokio::task::spawn_blocking(|| tracing::error!("blocking pool control line"));
    blocking.await.unwrap();
    let log = watch.capture.text();
    drop(watch);

    let mut kinds = secrets.kinds();
    kinds.sort_unstable();
    let mut expected = EXPECTED.to_vec();
    expected.sort_unstable();
    assert_eq!(kinds, expected, "every secret was driven through the hub");
    assert!(log.contains("capture control line"), "the log was captured");
    assert!(
        log.contains("blocking pool control line"),
        "and the blocking pool's"
    );
    assert!(log.contains(r#""level":"TRACE""#), "the log is at TRACE");
    for route in [
        "/graphql",
        "/metrics",
        "/daemon/v1/enrol",
        "/daemon/v1/renew",
    ] {
        assert!(
            log.contains(&format!(r#""path":"{route}""#)),
            "{route} is logged"
        );
    }
    // The log is not printed: it holds the secrets when this fails.
    let leaks: Vec<Leak> = secrets
        .0
        .iter()
        .flat_map(|s| forms::leaks(s, &log))
        .collect();
    let leaks: Vec<String> = leaks.iter().map(ToString::to_string).collect();
    assert!(
        leaks.is_empty(),
        "secrets in the process log:\n{}",
        leaks.join("\n")
    );
}

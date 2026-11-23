use anima_daemon::config::home::Home;
use anima_daemon::config::origin::Origin;
use anima_daemon::hub::{HubClient, TIMEOUT};
use anima_daemon::push::Pusher;
use anima_daemon::token_store::{Credential, TokenStore};
use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::points::{Kind, Point, SeriesKey, Window};
use daemon_core::wire::DaemonToken;
use tempfile::TempDir;
use uuid::Uuid;

use crate::fake_hub::FakeHub;

/// A multiple of 10 seconds, so every point's `at` is a window end.
pub const BASE: i64 = 1_800_000_000;

pub fn at(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(BASE + seconds, 0).expect("a time")
}

pub fn token(seed: u8) -> DaemonToken {
    DaemonToken::from_random([seed; 32])
}

/// Only what the token is bound to; the client talks to the fake hub on a local `http` address.
pub fn origin() -> Origin {
    Origin::parse("https://hub.example").expect("an origin")
}

pub fn device() -> Uuid {
    Uuid::from_u128(0xd1)
}

pub fn points(first: i64, count: i64) -> Vec<Point> {
    let key = SeriesKey::new(Kind::CpuPercent, None, None).expect("a key");
    let mut window = Window::new();
    let mut made = Vec::new();
    for step in first..=first + count {
        let value = f64::from(u32::try_from(step % 100).expect("small"));
        made.extend(window.add(at(step * 10), [(key.clone(), value)]));
    }
    made
}

pub struct Rig {
    pub pusher: Pusher,
    pub home: Home,
    _dir: TempDir,
}

pub fn rig(hub: &FakeHub, seed: u8, expires_in: TimeDelta) -> Rig {
    // The middle of the spread keeps every retry wait on its step.
    spread_rig(hub, seed, expires_in, || 0.5)
}

pub fn spread_rig(hub: &FakeHub, seed: u8, expires_in: TimeDelta, spread: fn() -> f64) -> Rig {
    let dir = tempfile::tempdir().expect("a folder");
    let home = Home::at(dir.path().join("daemon"));
    let store = TokenStore::open(&home);
    let credential = Credential {
        token: token(seed),
        expires_at: at(0) + expires_in,
        hub: origin(),
    };
    store.save(&credential).expect("stored");
    let client = HubClient::new(hub.url.clone(), TIMEOUT).expect("a client");
    Rig {
        pusher: Pusher::new(client, device(), credential, store).with_spread(spread),
        home,
        _dir: dir,
    }
}

pub fn year() -> TimeDelta {
    TimeDelta::days(365)
}

pub fn pushed_times(body: &serde_json::Value) -> Vec<i64> {
    body["points"]
        .as_array()
        .expect("points")
        .iter()
        .map(|point| {
            let text = point["at"].as_str().expect("at");
            DateTime::parse_from_rfc3339(text)
                .expect("rfc3339")
                .timestamp()
                - BASE
        })
        .collect()
}

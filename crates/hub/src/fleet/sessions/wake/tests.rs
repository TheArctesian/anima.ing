//! In-crate tests, for the loopback policy.

mod mutation;
mod refusals;
mod sending;
mod start;

use sqlx::PgPool;

use super::{Error, WakeSent, wake};
use crate::{
    fleet::{
        events::{ActorKind, Bus},
        sessions::tests::fixture::{DOCKER_HOST, LOOPBACK, PASSWORD, now, stored_text},
        ssh::tests::{Answer, Bench, Heard, Server, bench, reaching},
    },
    ids::DeviceId,
};

const PVE: &str = "0190a000-0000-7000-8002-000000000001";
const HOME: &str = "0190a000-0000-7000-8001-000000000001";
const MAC: &str = "aa:bb:cc:dd:ee:ff";
/// The relay's own MAC address, which no packet for `pve-01` may carry.
const RELAY_MAC: &str = "02:1b:3c:4d:5e:6f";
/// Text the relay writes; nothing the hub stores or returns may hold it (LOG-007).
const OUTPUT: &str = "Sending magic packet to 255.255.255.255:9";

fn pve() -> DeviceId {
    PVE.parse().unwrap()
}

fn exiting(status: u32) -> Answer {
    Answer::Exit {
        stdout: OUTPUT.as_bytes().to_vec(),
        stderr: OUTPUT.as_bytes().to_vec(),
        status,
    }
}

async fn relay(pool: &PgPool, answer: Answer) -> Bench {
    let bench = bench(pool, answer).await;
    set_mac(pool, PVE, Some(MAC)).await;
    set_mac(pool, DOCKER_HOST, Some(RELAY_MAC)).await;
    set_relay(pool, Some(DOCKER_HOST)).await;
    bench
}

async fn set_mac(pool: &PgPool, device: &str, mac: Option<&str>) {
    sqlx::query("update devices.devices set mac_address = $2::macaddr where id = $1::uuid")
        .bind(device)
        .bind(mac)
        .execute(pool)
        .await
        .unwrap();
}

async fn set_relay(pool: &PgPool, relay: Option<&str>) {
    sqlx::query("update devices.sites set wake_relay_device_id = $2::uuid where id = $1::uuid")
        .bind(HOME)
        .bind(relay)
        .execute(pool)
        .await
        .unwrap();
}

async fn woken(bench: &Bench, device: DeviceId) -> Result<WakeSent, Error> {
    let stores = (&bench.db, &bench.vault, &reaching(), &Bus::new(16));
    wake(stores, device, ActorKind::User, now()).await
}

async fn refused(bench: &Bench) -> Error {
    match woken(bench, pve()).await {
        Ok(sent) => panic!("the packet was sent: {sent:?}"),
        Err(error) => {
            for text in [error.to_string(), format!("{error:?}")] {
                assert_quiet(&text, &bench.server);
            }
            error
        }
    }
}

fn execs(server: &Server) -> Vec<String> {
    server
        .heard()
        .into_iter()
        .filter_map(|heard| match heard {
            Heard::Exec(line) => Some(line),
            _ => None,
        })
        .collect()
}

async fn events(pool: &PgPool) -> Vec<(String, String, Option<String>, String)> {
    sqlx::query_as("select severity, source, device_name, message from events.event_log")
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn one_error_event(pool: &PgPool, message: &str) {
    let error = (
        "error".to_owned(),
        "session".to_owned(),
        Some("pve-01".to_owned()),
        message.to_owned(),
    );
    assert_eq!(events(pool).await, [error]);
}

/// `mac`'s octets in decimal as `Debug` prints a byte array: `170, 187, 204, ...`.
fn decimal_octets(mac: &str) -> String {
    let octets: Vec<String> = mac
        .split(':')
        .map(|pair| u8::from_str_radix(pair, 16).unwrap().to_string())
        .collect();
    octets.join(", ")
}

/// A five-digit port also appears inside timings in a log line, so a plain `contains` is not enough.
fn holds_number(text: &str, number: &str) -> bool {
    text.match_indices(number).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + number.len()..].chars().next();
        !before.is_some_and(|each| each.is_ascii_digit())
            && !after.is_some_and(|each| each.is_ascii_digit())
    })
}

#[test]
fn a_port_is_found_as_a_whole_number_and_not_inside_a_timing() {
    assert!(holds_number("connect 127.0.0.1:37231 refused", "37231"));
    assert!(holds_number("37231", "37231"));
    assert!(!holds_number(r#""elapsed_secs":0.000372311"#, "37231"));
    assert!(!holds_number("372310 and 137231", "37231"));
}

/// `text` holds no address, port, MAC address (in hex or decimal octets), relay output or secret (LOG-007).
fn assert_quiet(text: &str, server: &Server) {
    let port = server.port.to_string();
    assert!(
        !holds_number(text, &port),
        "{text:?} holds the port {port:?}"
    );
    for secret in [
        LOOPBACK,
        MAC,
        RELAY_MAC,
        &decimal_octets(MAC),
        &decimal_octets(RELAY_MAC),
        OUTPUT,
        PASSWORD,
    ] {
        assert!(!text.contains(secret), "{text:?} holds {secret:?}");
    }
}

async fn assert_stored_quiet(bench: &Bench) {
    assert_quiet(&stored_text(&bench.db).await, &bench.server);
}

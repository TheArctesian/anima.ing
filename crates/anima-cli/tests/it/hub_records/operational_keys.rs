//! `anima secret` (K-E5 Section 2.2, OD-E23; CMD-014, CMD-024, CMD-025).

use serde_json::{Value, json};

use crate::kit::Anima;
use crate::kit::fake_hub::{FakeHub, data};
use crate::kit::terminal::Terminal;

const VALUE: &str = "cf-token-fixture-9d1e";
const DEVICE: &str = "0190a000-0000-7000-8001-000000000001";

fn signed_in() -> Anima {
    Anima::new().with("ANIMA_TOKEN", "login-token-for-secret-tests")
}

fn key() -> Value {
    json!({
        "id": "0190a000-0000-7000-9700-000000000001", "name": "cloudflare-dns",
        "provider": "cloudflare", "purpose": "DNS updates", "instructions": "Dashboard > Tokens",
        "providerLink": null, "rotationDays": 90, "expiresAt": null, "due": "CURRENT",
        "activeVersion": 2, "lastRotatedAt": "2026-11-20T10:00:00+00:00", "lastRotatedBy": "op",
        "createdAt": "2026-11-01T10:00:00+00:00", "createdBy": "op",
        "versions": [{ "number": 2, "state": "ACTIVE", "createdAt": "2026-11-20T10:00:00+00:00",
                       "createdBy": "op", "endedAt": null, "rotatedAtProvider": true }],
        "consumers": [{ "deviceId": DEVICE, "workload": "caddy",
                        "grantedAt": "2026-11-01T10:00:00+00:00", "grantedBy": "op",
                        "heldVersion": 1, "heldSeenAt": "2026-11-20T09:00:00+00:00",
                        "fetchedVersion": 1, "fetchedAt": "2026-11-01T11:00:00+00:00" }]
    })
}

fn ending() -> Value {
    json!({ "key": key(), "endedVersion": 1, "stillHeldBy": [
        { "deviceId": DEVICE, "workload": "caddy", "heldVersion": 1,
          "heldSeenAt": "2026-11-20T09:00:00+00:00" }] })
}

#[tokio::test]
async fn cmd_025_add_reads_the_value_from_a_pipe_and_never_prints_it() {
    let hub = FakeHub::start().await;
    hub.answer("SecretAdd", data(json!({ "createOperationalKey": key() })));
    let run = signed_in()
        .run_with_input(
            &[
                "secret",
                "add",
                "cloudflare-dns",
                "--provider",
                "cloudflare",
                "--purpose",
                "DNS updates",
                "--hub",
                &hub.origin,
            ],
            &format!("{VALUE}\n"),
        )
        .await;
    assert_eq!(run.code, 0, "{run:?}");
    let input = &hub.seen()[0].variables["input"];
    assert_eq!(input["value"], VALUE);
    assert_eq!(input["provider"], "cloudflare");
    run.assert_never_shows(VALUE);
    assert!(run.stdout.contains("cloudflare-dns"), "{}", run.stdout);
}

#[tokio::test]
async fn cmd_025_add_asks_at_a_terminal_with_no_echo() {
    let hub = FakeHub::start().await;
    hub.answer("SecretAdd", data(json!({ "createOperationalKey": key() })));
    let mut terminal = Terminal::start(
        &signed_in(),
        &[
            "secret",
            "add",
            "cloudflare-dns",
            "--provider",
            "cloudflare",
            "--purpose",
            "DNS",
            "--hub",
            &hub.origin,
        ],
    );
    terminal.expect("The value of the key").await;
    terminal.type_line(VALUE).await;
    let (code, screen) = terminal.finish().await;
    assert_eq!(code, 0, "{screen}");
    assert!(!screen.contains(VALUE), "{screen}");
    assert_eq!(hub.seen()[0].variables["input"]["value"], VALUE);
}

#[tokio::test]
async fn cmd_025_no_flag_carries_the_value() {
    let hub = FakeHub::start().await;
    for flag in ["--value", "--secret"] {
        let run = signed_in()
            .run(&[
                "secret",
                "rotate",
                "cloudflare-dns",
                flag,
                VALUE,
                "--hub",
                &hub.origin,
            ])
            .await;
        assert_eq!(run.code, 2, "{run:?}");
    }
    assert!(hub.seen().is_empty());
}

#[tokio::test]
async fn k_e5_rotate_says_recorded_unless_the_provider_changed_and_names_the_holders() {
    let hub = FakeHub::start().await;
    hub.answer(
        "SecretRotate",
        data(json!({ "rotateOperationalKey": ending() })),
    );
    let run = signed_in()
        .run_with_input(
            &[
                "secret",
                "rotate",
                "cloudflare-dns",
                "--yes",
                "--hub",
                &hub.origin,
            ],
            VALUE,
        )
        .await;
    assert_eq!(run.code, 0, "{run:?}");
    let sent = &hub.seen()[0].variables;
    assert_eq!(
        (sent["value"].clone(), sent["rotatedAtProvider"].clone()),
        (json!(VALUE), json!(false))
    );
    assert!(run.stderr.contains("recorded"), "{}", run.stderr);
    assert!(run.stdout.contains("Still held by"), "{}", run.stdout);
    run.assert_never_shows(VALUE);
}

#[tokio::test]
async fn cmd_024_grant_and_revoke_need_a_confirmation() {
    let hub = FakeHub::start().await;
    for args in [
        vec![
            "secret",
            "grant",
            "cloudflare-dns",
            "--device",
            DEVICE,
            "--workload",
            "caddy",
        ],
        vec!["secret", "revoke", "cloudflare-dns"],
        vec!["secret", "remove", "cloudflare-dns"],
    ] {
        let mut args = args;
        args.extend(["--hub", &hub.origin]);
        let run = signed_in().run(&args).await;
        assert_eq!(run.code, 2, "{run:?}");
        assert!(run.stderr.contains("--yes"), "{}", run.stderr);
    }
    assert!(hub.seen().is_empty(), "nothing reached the hub");
}

#[tokio::test]
async fn k_e5_set_changes_only_what_is_given() {
    let hub = FakeHub::start().await;
    hub.answer("SecretShow", data(json!({ "operationalKey": key() })));
    hub.answer("SecretSet", data(json!({ "updateOperationalKey": key() })));
    let run = signed_in()
        .run(&[
            "secret",
            "set",
            "cloudflare-dns",
            "--purpose",
            "DNS and certificates",
            "--no-rotation-days",
            "--hub",
            &hub.origin,
        ])
        .await;
    assert_eq!(run.code, 0, "{run:?}");
    let input = &hub.seen()[1].variables["input"];
    assert_eq!(input["purpose"], "DNS and certificates");
    assert_eq!(input["provider"], "cloudflare");
    assert_eq!(input["instructions"], "Dashboard > Tokens");
    assert!(input["rotationDays"].is_null(), "{input}");
}

#[tokio::test]
async fn k_e5_show_of_an_unknown_name_is_not_found() {
    let hub = FakeHub::start().await;
    hub.answer("SecretShow", data(json!({ "operationalKey": null })));
    let run = signed_in()
        .run(&["secret", "show", "nothing", "--hub", &hub.origin])
        .await;
    assert_eq!(run.code, 3, "{run:?}");
}

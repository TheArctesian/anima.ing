//! The accounts of a batch (AIS-022 as THR-149 changed it): a key, the address, a plan and
//! limit windows leave the machine, never the identifier of an account.

use agent_threads::drive::AccountFacts;
use base64::Engine as _;
use daemon_core::agent_usage::account::account_key;
use daemon_core::agent_usage::{Source, UsageState};
use serde_json::json;

use super::UsageBacklog;
use crate::sampling::agent_usage::{ProgramAccount, UsageReport};

const SALT: [u8; 32] = [7; 32];
const ID: &str = "user-7f3a9c-ID";
const NAME: &str = "person-7f3a9c@example.com";

fn codex(facts: serde_json::Value) -> ProgramAccount {
    ProgramAccount {
        source: Source::Codex,
        facts: serde_json::from_value::<AccountFacts>(facts).expect("facts"),
    }
}

fn signed_in() -> ProgramAccount {
    codex(json!({
        "id": ID, "name": NAME, "plan": "pro",
        "limits": [{ "window": "300 min", "used_percent": 35, "resets_at": 1_795_000_000, "reached": false }]
    }))
}

fn report(accounts: Option<Vec<ProgramAccount>>) -> UsageReport {
    UsageReport {
        state: UsageState::Reading,
        session_days: Vec::new(),
        accounts,
    }
}

fn salt_text() -> String {
    base64::engine::general_purpose::STANDARD.encode(SALT)
}

/// A daemon whose setting `sendAccounts` is false reports no accounts: its batches carry no
/// accounts member at all, so the hub neither gets keys nor sends its salt.
#[test]
fn ais_022_with_no_accounts_reported_a_batch_has_no_accounts_member() {
    let mut backlog = UsageBacklog::default();
    backlog.take(report(None));
    let sent = backlog
        .outgoing()
        .expect("the first batch announces the state");
    assert_eq!(sent.accounts, None);
    let text = serde_json::to_string(&sent).expect("json");
    assert!(!text.contains("accounts"), "{text}");
}

#[test]
fn ais_022_with_no_salt_an_empty_list_asks_for_it_then_the_key_goes() {
    let mut backlog = UsageBacklog::default();
    backlog.take(report(Some(vec![signed_in()])));
    let asking = backlog.outgoing().expect("a batch");
    assert_eq!(asking.accounts, Some(Vec::new()), "no salt: an empty list");
    backlog.done();
    backlog.take_salt(Some(&salt_text()));
    let sent = backlog.outgoing().expect("the accounts");
    let accounts = sent.accounts.expect("the accounts member");
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].source, "codex");
    assert_eq!(accounts[0].key, account_key(&SALT, Source::Codex, ID));
    assert_eq!(
        accounts[0].address.as_ref().map(|one| one.reveal()),
        Some(NAME)
    );
    assert_eq!(accounts[0].plan.as_deref(), Some("pro"));
    assert_eq!(accounts[0].limits[0].window, "300 min");
    assert_eq!(accounts[0].limits[0].used_percent, Some(35));
}

#[test]
fn thr_149_the_address_leaves_and_the_identifier_never() {
    let mut backlog = UsageBacklog::default();
    backlog.take_salt(Some(&salt_text()));
    backlog.take(report(Some(vec![signed_in()])));
    let sent = backlog.outgoing().expect("a batch");
    let text = serde_json::to_string(&sent).expect("json");
    assert!(text.contains(NAME), "{text}");
    assert!(!text.contains(ID), "{text}");
    assert!(!text.contains("user-"), "{text}");
}

/// Codex gives the name of a workspace when it has no address: that name stays at the machine.
#[test]
fn thr_149_a_name_that_is_not_an_address_never_leaves() {
    let mut backlog = UsageBacklog::default();
    backlog.take_salt(Some(&salt_text()));
    let workspace = codex(json!({ "id": ID, "name": "Team 7f3a9c", "plan": "team", "limits": [] }));
    backlog.take(report(Some(vec![workspace])));
    let sent = backlog.outgoing().expect("a batch");
    let accounts = sent.accounts.clone().expect("accounts");
    assert_eq!(accounts[0].address, None);
    let text = serde_json::to_string(&sent).expect("json");
    assert!(!text.contains("7f3a9c"), "{text}");
}

/// THR-149 point 7: what the pusher could log of a batch, its `Debug`, holds no address.
#[test]
fn thr_149_the_debug_of_a_batch_holds_no_address() {
    let mut backlog = UsageBacklog::default();
    backlog.take_salt(Some(&salt_text()));
    backlog.take(report(Some(vec![signed_in()])));
    let sent = backlog.outgoing().expect("a batch");
    let shown = format!("{sent:?} {:?}", signed_in());
    assert!(shown.contains("Address(..)"), "{shown}");
    assert!(!shown.contains("person-7f3a9c"), "{shown}");
    assert!(!shown.contains(ID), "{shown}");
}

#[test]
fn ais_022_accounts_go_again_only_when_they_change() {
    let mut backlog = UsageBacklog::default();
    backlog.take_salt(Some(&salt_text()));
    backlog.take(report(Some(vec![signed_in()])));
    assert!(backlog.outgoing().is_some());
    backlog.done();
    backlog.take(report(Some(vec![signed_in()])));
    assert_eq!(backlog.outgoing(), None, "nothing new");
    backlog.take(report(None));
    assert_eq!(
        backlog.outgoing(),
        None,
        "no runner: the sent accounts stay"
    );
    let mut fuller = signed_in();
    fuller.facts.limits[0].used_percent = Some(90);
    backlog.take(report(Some(vec![fuller])));
    let sent = backlog.outgoing().expect("the change");
    assert_eq!(
        sent.accounts.expect("accounts")[0].limits[0].used_percent,
        Some(90)
    );
}

#[test]
fn ais_022_a_program_with_no_identifier_or_signed_out_sends_no_account() {
    let mut backlog = UsageBacklog::default();
    backlog.take_salt(Some(&salt_text()));
    let claude = ProgramAccount {
        source: Source::ClaudeCode,
        facts: serde_json::from_value(json!({
            "limits": [{ "window": "five_hour", "used_percent": 50, "resets_at": null, "reached": false }]
        }))
        .expect("facts"),
    };
    let out = codex(json!({ "limits": [], "signed_out": true }));
    backlog.take(report(Some(vec![claude, out])));
    let sent = backlog.outgoing().expect("a batch");
    assert_eq!(sent.accounts, Some(Vec::new()));
}

#[test]
fn ais_022_a_salt_that_does_not_read_is_ignored() {
    let mut backlog = UsageBacklog::default();
    backlog.take_salt(Some("not base64!"));
    backlog.take_salt(Some(
        &base64::engine::general_purpose::STANDARD.encode([1_u8; 8]),
    ));
    backlog.take(report(Some(vec![signed_in()])));
    assert_eq!(
        backlog.outgoing().expect("a batch").accounts,
        Some(Vec::new())
    );
}

/// With `sendAccounts` false the task reports an empty list: one batch says so, and the hub
/// deletes what it had of the device (THR-149); then nothing more goes.
#[test]
fn thr_149_turned_off_an_empty_list_goes_once() {
    let mut backlog = UsageBacklog::default();
    backlog.take_salt(Some(&salt_text()));
    backlog.take(report(Some(vec![signed_in()])));
    assert!(backlog.outgoing().is_some());
    backlog.done();
    backlog.take(report(Some(Vec::new())));
    let sent = backlog.outgoing().expect("the empty list");
    assert_eq!(sent.accounts, Some(Vec::new()));
    backlog.done();
    backlog.take(report(Some(Vec::new())));
    assert_eq!(backlog.outgoing(), None, "said once");
}

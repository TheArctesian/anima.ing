//! The model work in the running daemon (L1-6d): the claims at the rhythm of the switch at the
//! machine (USE-004, K10 Section 4), the sweep before the first claim (SF-259, SF-317), and the
//! end of each server when the model work ends (SF-259, R-340).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anima_daemon::config::home::Home;
use anima_daemon::hub::{HubClient, TIMEOUT};
use anima_daemon::runtime::model_work::{Hub, ModelWork};
use anima_daemon::token_store::{Credential, TokenStore};
use anima_daemon::work::switch;
use serde_json::{Value, json};

use crate::fake_hub::{FakeHub, Reply};
use crate::kit::{at, origin, token, year};
use crate::model_work::kit::{contained, node, release};
use crate::model_work::servers::{SERVER, gone, installed_node};

fn hub(fake: &FakeHub, home: &Home) -> Hub {
    home.ensure().expect("the daemon folder");
    let tokens = TokenStore::open(home);
    let credential = Credential {
        token: token(3),
        expires_at: at(0) + year(),
        hub: origin(),
    };
    tokens.save(&credential).expect("stored");
    let client = HubClient::new(fake.url.clone(), TIMEOUT).expect("a client");
    Hub {
        client: Arc::new(client),
        origin: origin(),
        tokens,
    }
}

/// The bodies of the route `route` once there are `count` of them, or after five seconds.
async fn seen(fake: &FakeHub, route: &str, count: usize) -> Vec<Value> {
    for _ in 0..50 {
        let bodies: Vec<Value> = fake.seen(route).into_iter().map(|s| s.body).collect();
        if bodies.len() >= count {
            return bodies;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    fake.seen(route).into_iter().map(|s| s.body).collect()
}

fn home_in(root: &Path) -> Home {
    Home::at(root.join("daemon"))
}

#[tokio::test]
async fn use_004_a_machine_switched_off_claims_once_at_its_start_and_says_so() {
    let fake = FakeHub::start().await;
    let root = tempfile::tempdir().expect("a folder");
    let home = home_in(root.path());
    let hub = hub(&fake, &home);
    switch::set(home.dir(), false).expect("off");

    let work = ModelWork::start(node(root.path()), hub, home);
    let claims = seen(&fake, "claim", 1).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    work.end().await;

    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0]["switch"], json!("off"));
    assert_eq!(
        fake.seen("claim").len(),
        1,
        "no second claim within the hour"
    );
}

#[tokio::test]
async fn use_004_the_switch_is_on_by_default_and_the_claim_sends_the_token() {
    let fake = FakeHub::start().await;
    let root = tempfile::tempdir().expect("a folder");
    let home = home_in(root.path());
    let hub = hub(&fake, &home);

    let work = ModelWork::start(node(root.path()), hub, home);
    let claims = seen(&fake, "claim", 1).await;
    work.end().await;

    assert_eq!(claims[0]["switch"], json!("on"));
    assert_eq!(claims[0]["kinds"], json!(["model"]));
    assert_eq!(
        fake.seen("claim")[0].bearer.as_deref(),
        Some(token(3).expose())
    );
}

#[tokio::test]
async fn sf_317_an_install_that_an_earlier_run_left_is_removed_before_the_first_claim() {
    let fake = FakeHub::start().await;
    let root = tempfile::tempdir().expect("a folder");
    let home = home_in(root.path());
    let hub = hub(&fake, &home);
    let node = node(root.path());
    let runtime = node.folders.runtimes.join("llama_server");
    let left = runtime.join(".unpack-a1b2c3");
    std::fs::create_dir_all(left.join("bin")).expect("a left folder");
    std::fs::write(left.join("bin/llama-server"), b"x").expect("a file");
    let installed = runtime.join("b8100-cpu");
    std::fs::create_dir_all(&installed).expect("an install");
    let outside = root.path().join("outside");
    std::fs::create_dir_all(&outside).expect("a folder");
    std::fs::write(outside.join("kept"), b"x").expect("a file");
    let link = runtime.join(".unpack-link");
    std::os::unix::fs::symlink(&outside, &link).expect("a link");
    fake.queue_claim(Reply::no_content());

    let work = ModelWork::start(node, hub, home);
    let gone_before_claim = !left.exists();
    work.end().await;

    assert!(gone_before_claim, "removed before the loop starts");
    assert!(installed.is_dir(), "a complete install stays");
    assert!(link.symlink_metadata().is_ok(), "a link is not removed");
    assert!(outside.join("kept").is_file(), "a link is not followed");
}

fn start_item() -> String {
    let start = json!({
        "server": SERVER, "runtime": "llama_server", "version": "b8100", "backend": "cpu", "settings": {},
    });
    json!({
        "id": "0192f6aa-0000-7000-8000-000000000077", "type": "server_start", "kind": "model", "attempt": 1,
        "lease": { "until": "2026-11-20T10:01:30Z" },
        "limits": { "seconds": 600, "idleSeconds": 60, "memoryBytes": 1, "diskBytes": 1, "orphanSeconds": 60 },
        "project": null, "secrets": [], "parameters": start, "actor": "User",
    })
    .to_string()
}

fn child_in(file: &Path) -> rustix::process::Pid {
    let text = std::fs::read_to_string(file).expect("the child of the server");
    rustix::process::Pid::from_raw(text.trim().parse().expect("a pid")).expect("a pid")
}

#[tokio::test]
async fn sf_259_each_server_ends_when_the_model_work_ends() {
    let Some(_isolation) = contained() else {
        return;
    };
    let fake = FakeHub::start().await;
    let root = tempfile::tempdir().expect("a folder");
    let home = home_in(root.path());
    let hub = hub(&fake, &home);
    let node = installed_node(root.path(), "loopback", 42_400);
    let child_file: PathBuf = node.folders.release(&release()).join("bin/child");
    fake.queue_claim(Reply::json(200, &start_item()));

    let work = ModelWork::start(node, hub, home);
    let reports = seen(&fake, "report", 2).await;
    assert_eq!(reports[1]["state"], json!("succeeded"), "{reports:?}");
    let child = child_in(&child_file);
    work.end().await;

    assert!(gone(child).await, "the server still runs after the end");
}

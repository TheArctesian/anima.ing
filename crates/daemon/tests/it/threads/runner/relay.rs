//! TH118 through the public runner socket: opt-in, refusal, and persisted non-secret choice.

use agent_threads::control::{Action, Outcome, ProviderAddress, Refusal};

use super::Kit;

fn start(kit: &Kit, relay: bool) -> Action {
    let mut call = serde_json::json!({
        "call": "start", "program": "claude_code", "folder": kit.path("work"),
        "mode": "full_access", "prompt": "hello"
    });
    if relay {
        call["provider_address"] = "relay".into();
    }
    serde_json::from_value(call).expect("start call")
}

#[tokio::test]
async fn th118_the_route_is_opt_in_fixed_and_kept_without_a_secret_after_restart() {
    let mut kit = Kit::new();
    let mut client = kit.client().await;
    let asked = client.call(start(&kit, false)).await;
    let Outcome::Started { thread: direct, .. } = client.done(asked).await else {
        panic!("direct start")
    };
    let asked = client.call(start(&kit, true)).await;
    let Outcome::Started {
        thread: relayed, ..
    } = client.done(asked).await
    else {
        panic!("relay start")
    };
    drop(client);
    kit.end_runner();
    let record = std::fs::read_to_string(kit.threads().join("threads.json")).expect("record");
    assert!(!record.contains("http://127.0.0.1:"));
    kit.start();
    let mut client = kit.client().await;
    let asked = client.call(Action::List).await;
    let Outcome::Threads { threads, .. } = client.done(asked).await else {
        panic!("list")
    };
    assert_eq!(
        threads
            .iter()
            .find(|row| row.id == direct)
            .expect("direct row")
            .provider_address,
        None
    );
    assert_eq!(
        threads
            .iter()
            .find(|row| row.id == relayed)
            .expect("relay row")
            .provider_address,
        Some(ProviderAddress::Relay)
    );
    for thread in [direct, relayed] {
        let stop = client.call(Action::Stop { thread }).await;
        client.done(stop).await;
    }
    let log = std::fs::read_to_string(kit.threads().join("runner.log")).expect("log");
    assert!(log.contains("pass-through started"));
    assert!(!log.contains("http://127.0.0.1:"));
}

#[tokio::test]
async fn th118_the_runner_returns_the_settings_refusal_in_words() {
    let kit = Kit::new();
    let settings = kit.path("work/.claude");
    std::fs::create_dir(&settings).expect("settings folder");
    std::fs::write(
        settings.join("settings.json"),
        r#"{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:1"}}"#,
    )
    .expect("synthetic settings");
    let mut client = kit.client().await;
    let asked = client.call(start(&kit, true)).await;
    let Outcome::Refused {
        why: Refusal::ProviderAddress { said },
    } = client.done(asked).await
    else {
        panic!("provider refusal");
    };
    assert!(said.contains("ANTHROPIC_BASE_URL"));
    let list = client.call(Action::List).await;
    let Outcome::Threads { threads, .. } = client.done(list).await else {
        panic!("list")
    };
    assert!(
        threads.is_empty(),
        "a refused start leaves no running thread"
    );
    let mut invalid = serde_json::to_value(start(&kit, true)).expect("call");
    invalid["provider_address"] = "http://example.invalid".into();
    assert!(
        serde_json::from_value::<Action>(invalid).is_err(),
        "no arbitrary upstream"
    );
}

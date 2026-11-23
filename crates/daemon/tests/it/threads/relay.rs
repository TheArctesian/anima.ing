//! TH118 at the holder boundary: secret lifetime, reconnection, and child-only settings.

use agent_threads::drive::Program;
use anima_daemon::threads::wire::{Action, Event, Outcome, Refusal};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use super::{Holder, exited, start, texts};

fn relayed(thread: &str, script: &str, program: Program) -> Action {
    let mut action = start(thread, "/bin/sh", &["-c", script]);
    if let Action::Start { relay, .. } = &mut action {
        *relay = Some(program);
    }
    action
}

// The made child gives its URL to this test in memory, never through the holder's output or
// a file. Upgrade requests prove secret acceptance without forwarding to a real provider.
async fn collect(listener: tokio::net::TcpListener) -> String {
    let (mut connection, _) = listener.accept().await.expect("child connects");
    let mut request = Vec::new();
    let mut byte = [0];
    while !request.ends_with(b"\r\n\r\n") {
        connection.read_exact(&mut byte).await.expect("header");
        request.push(byte[0]);
        assert!(request.len() < 4096);
    }
    let head = String::from_utf8(request).expect("ASCII header");
    let length: usize = head
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")
                .map(str::to_owned)
        })
        .expect("content length")
        .parse()
        .expect("number");
    assert!(length < 256);
    let mut body = vec![0; length];
    connection.read_exact(&mut body).await.expect("URL");
    connection
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        .await
        .expect("answer");
    String::from_utf8(body).expect("ASCII URL")
}

async fn status(address: &str) -> u16 {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(super::WAIT)
        .build()
        .expect("client")
        .get(format!("{address}/v1/messages"))
        .header("upgrade", "websocket")
        .send()
        .await
        .expect("local reply")
        .status()
        .as_u16()
}

#[tokio::test]
async fn th118_the_holder_keeps_the_relay_across_reconnection_and_revokes_it_at_exit() {
    let collector = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("collector");
    let target = collector.local_addr().expect("address");
    let receiving = tokio::spawn(collect(collector));
    let script = format!(
        "/usr/bin/curl --silent --noproxy '*' --data-binary \"$ANTHROPIC_BASE_URL\" http://{target}/; echo \"$ANTHROPIC_BASE_URL\"; read line; echo \"$ANTHROPIC_BASE_URL\""
    );
    let holder = Holder::start(0);
    let mut link = holder.link().await;
    let id = link
        .send(relayed("lease", &script, Program::ClaudeCode))
        .await;
    assert!(matches!(link.answer(id).await, Outcome::Started { .. }));
    let address = tokio::time::timeout(super::WAIT, receiving)
        .await
        .expect("in time")
        .expect("collected");
    assert!(address.starts_with("http://127.0.0.1:"));
    assert_eq!(status(&address).await, 501);
    drop(link);
    assert_eq!(status(&address).await, 501, "works while daemon is absent");
    let mut link = holder.link().await;
    let hello = link
        .send(Action::Hello {
            version: anima_daemon::threads::wire::VERSION,
        })
        .await;
    link.answer(hello).await;
    assert_eq!(
        status(&address).await,
        501,
        "same secret and port after reconnect"
    );
    link.send(Action::CloseInput {
        thread: "lease".into(),
    })
    .await;
    let events = link.until(exited("lease")).await;
    assert_eq!(status(&address).await, 404, "exit revokes the secret");
    let serialized = serde_json::to_string(&events).expect("events");
    assert!(!serialized.contains(&address));
    let secret = address.rsplit('/').next().expect("secret");
    assert!(
        !serialized.contains(secret),
        "even the last output is redacted"
    );
    assert!(
        texts(&events)
            .iter()
            .any(|(_, text)| text.contains("[relay]"))
    );
    let record = std::fs::read_to_string(holder.dir.join("holder.json")).expect("holder record");
    assert!(!record.contains(secret));
}

#[tokio::test]
async fn th118_a_conflicting_or_unreadable_setting_refuses_before_start() {
    let holder = Holder::start(0);
    let folder = tempfile::tempdir().expect("folder");
    std::fs::create_dir(folder.path().join(".claude")).expect("settings folder");
    let path = folder.path().join(".claude/settings.json");
    let mut link = holder.link().await;
    for settings in [
        r#"{"env":{"ANTHROPIC_BASE_URL":"https://example.invalid"}}"#,
        "broken json",
    ] {
        std::fs::write(&path, settings).expect("synthetic settings");
        let mut action = relayed("conflict", "echo should-not-start", Program::ClaudeCode);
        if let Action::Start { folder: chosen, .. } = &mut action {
            *chosen = folder.path().display().to_string();
        }
        let id = link.send(action).await;
        assert_eq!(
            link.answer(id).await,
            Outcome::Refused {
                why: Refusal::ProviderConflict
            }
        );
    }
    std::fs::write(path, "{}").expect("clear settings");
    let mut action = relayed("conflict", "echo started", Program::ClaudeCode);
    if let Action::Start { folder: chosen, .. } = &mut action {
        *chosen = folder.path().display().to_string();
    }
    let id = link.send(action).await;
    assert!(matches!(link.answer(id).await, Outcome::Started { .. }));
    let events = link.until(exited("conflict")).await;
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Exited { .. }))
    );
}

#[tokio::test]
async fn th118_a_failed_spawn_releases_the_lease_for_a_retry() {
    let holder = Holder::start(0);
    let mut link = holder.link().await;
    let mut bad = relayed("retry", "echo good", Program::Codex);
    if let Action::Start { program, .. } = &mut bad {
        *program = "/does-not-exist-th118".to_owned();
    }
    let id = link.send(bad).await;
    assert_eq!(
        link.answer(id).await,
        Outcome::Refused {
            why: Refusal::StartFailed
        }
    );
    let id = link
        .send(relayed(
            "retry",
            "printf '%s\\n' \"$0\" \"$1\"",
            Program::Codex,
        ))
        .await;
    assert!(matches!(link.answer(id).await, Outcome::Started { .. }));
    let lines = texts(&link.until(exited("retry")).await);
    assert_eq!(lines[0].1, "-c");
    assert!(
        lines[1]
            .1
            .starts_with("openai_base_url=\"http://127.0.0.1:")
    );
    assert_eq!(lines[1].1, "openai_base_url=\"http://127.0.0.1:0/v1\"");
}

#[tokio::test]
async fn th118_codex_receives_private_routing_on_start_resume_and_fork() {
    let holder = Holder::start(0);
    let mut connection = holder.link().await;
    let id = connection
        .send(relayed(
            "private",
            "while IFS= read -r line; do printf '%s\\n' \"$line\"; done",
            Program::Codex,
        ))
        .await;
    assert!(matches!(
        connection.answer(id).await,
        Outcome::Started { .. }
    ));
    for method in ["thread/start", "thread/resume", "thread/fork"] {
        let line = serde_json::json!({"id":1,"method":method,"params":{"config":{"unrelated":"kept","openai_base_url":"http://127.0.0.1:1"}}}).to_string();
        let id = connection
            .send(Action::Write {
                thread: "private".into(),
                line,
            })
            .await;
        assert_eq!(connection.answer(id).await, Outcome::Ok);
    }
    drop(connection);
    let mut connection = holder.link().await;
    let id = connection
        .send(Action::Write {
            thread: "private".into(),
            line: r#"{"id":2,"method":"thread/resume","params":{}}"#.into(),
        })
        .await;
    assert_eq!(connection.answer(id).await, Outcome::Ok);
    connection
        .send(Action::CloseInput {
            thread: "private".into(),
        })
        .await;
    let lines = texts(&connection.until(exited("private")).await);
    assert_eq!(lines.len(), 4);
    for (index, (_, line)) in lines.iter().enumerate() {
        let message: serde_json::Value = serde_json::from_str(line).expect("child input");
        let config = &message["params"]["config"];
        let address = config["openai_base_url"].as_str().expect("private URL");
        assert!(address.starts_with("http://127.0.0.1:"));
        assert!(address.ends_with("/[relay]/v1"));
        if index < 3 {
            assert_eq!(config["unrelated"], "kept");
        }
    }
}

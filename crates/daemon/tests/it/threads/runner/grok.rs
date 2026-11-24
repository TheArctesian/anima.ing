use std::os::unix::fs::PermissionsExt as _;

use agent_threads::control::{Action, Change, Outcome};
use agent_threads::drive::{DriveEvent, Mode, Program};

use super::{Kit, change};

const GROK: &str = r#"#!/usr/bin/perl
$| = 1;
if ($ARGV[0] eq '--version') { print "grok 1.0\n"; exit; }
while (my $line = <STDIN>) {
    if ($line =~ /"method":"initialize"/) {
        print qq({"jsonrpc":"2.0","id":"grok-initialize","result":{"protocolVersion":1}}\n);
    } elsif ($line =~ /"method":"session\/new"/) {
        print qq({"jsonrpc":"2.0","id":"grok-session","result":{"sessionId":"made"}}\n);
    } elsif ($line =~ /"id":"grok-turn-1"/) {
        open(my $file, '>', 'first-running'); close($file);
        if (fork() == 0) {
            while (!-e 'end-first') { select(undef, undef, undef, 0.02); }
            print qq({"jsonrpc":"2.0","id":"grok-turn-1","result":{"stopReason":"end_turn"}}\n);
            exit;
        }
    } elsif ($line =~ /"method":"session\/prompt"/) {
        open(my $file, '>', 'second-sent'); close($file);
    }
}
"#;

#[tokio::test]
async fn grok_queued_turn_waits_when_its_identity_cannot_be_saved() {
    let mut kit = Kit::unstarted(super::CLAUDE);
    let program = kit.path("bin/grok");
    std::fs::write(&program, GROK).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    kit.start();
    let mut client = kit.client().await;
    let watch = client.call(Action::Watch).await;
    assert_eq!(client.done(watch).await, Outcome::Ok);
    let start = client
        .call(Action::Start {
            provider_address: None,
            program: Program::Grok,
            folder: kit.path("work"),
            mode: Mode::FullAccess,
            model: None,
            effort: None,
            speed: None,
            prompt: "first".into(),
            program_path: Some(program),
            draft: None,
            attachments: Vec::new(),
            worktree: false,
            base: None,
        })
        .await;
    let Outcome::Started { thread, .. } = client.done(start).await else {
        panic!("started")
    };
    let active = kit.path("work/first-running");
    for _ in 0..200 {
        if active.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(active.exists(), "the first turn is running");
    let queued = client
        .call(Action::Queue {
            thread,
            text: "second".into(),
        })
        .await;
    assert_eq!(client.done(queued).await, Outcome::Ok);
    // Make the next real record write fail without touching permissions of the test process.
    std::fs::create_dir(kit.threads().join("threads.json.new")).unwrap();
    std::fs::write(kit.path("work/end-first"), "").unwrap();
    client
        .until(|reply| {
            matches!(change(reply), Some(Change::Said {
        event: DriveEvent::Notice { text }
    }) if text.contains("record") && text.contains("did not go"))
        })
        .await;
    client
        .until(|reply| {
            matches!(change(reply), Some(Change::State { row, .. })
        if row.queued.iter().any(|prompt| prompt.text == "second"))
        })
        .await;
    assert!(!kit.path("work/second-sent").exists());
}

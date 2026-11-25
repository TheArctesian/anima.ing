use std::path::PathBuf;
use std::time::Duration;

use agent_threads::control::{Action, Outcome};
use agent_threads::git::read::{self, Read, Request};
use agent_threads::git::{GitAction, GitOutcome, Scope};
use serde_json::Value;
use uuid::Uuid;

use super::{Kit, Mode, Program, one_commit};

const AGENT: &str = include_str!("read_agent.pl");

#[tokio::test]
async fn tc7_w3_only_live_holder_children_read_their_own_worktree() {
    let mut kit = Kit::with(AGENT);
    let mut person = kit.client().await;
    if !capable() {
        let external = ask(&mut person, "work", Read::Status).await;
        assert_eq!(external, denied(read::Refusal::Unsupported));
        eprintln!("Kernel has no socket-bound pidfs support; fixed refusal verified.");
        return;
    }
    let first = start(&kit, &mut person, "work").await;
    let second = start(&kit, &mut person, "other").await;
    assert_eq!(
        ask(&mut person, "work", Read::Status).await,
        denied(read::Refusal::Unbound)
    );

    let receipt = review(&mut person, first, "work").await;
    let ref_path = kit
        .threads()
        .join("git-records")
        .join(first.to_string())
        .join("work.git")
        .join(format!("refs/anima/checkpoints/{first}/reviewed-text"));
    let held = std::fs::read_to_string(&ref_path).expect("the W2 review ref");
    assert_eq!(held.trim(), receipt);
    std::fs::write(
        kit.path(&format!(".anima/worktrees/{first}/work/a")),
        "fixture-sentinel-v2\n",
    )
    .expect("new change");

    let output = query(&kit, &mut person, first, "work").await;
    assert_probe(&output);
    assert_eq!(
        std::fs::read_to_string(ref_path).expect("the W2 review ref"),
        held
    );
    assert_eq!(
        query(&kit, &mut person, second, "other").await[3]["result"]["result"]["why"],
        "no_worktree"
    );

    kit.end_runner();
    kit.start();
    let mut person = kit.client().await;
    assert_probe(&query(&kit, &mut person, first, "work").await);

    let stopped = person.call(Action::Stop { thread: first }).await;
    assert!(matches!(
        person.done(stopped).await,
        Outcome::Stopped { .. } | Outcome::Ok
    ));
    assert_eq!(
        ask(&mut person, "work", Read::Status).await,
        denied(read::Refusal::Unbound)
    );
}

fn assert_probe(output: &Value) {
    assert_eq!(
        output[0]["result"]["result"]["result"], "status",
        "{output}"
    );
    assert_eq!(
        output[1]["result"]["result"]["checkpoints"][0]["turn"], 0,
        "{output}"
    );
    assert_eq!(
        output[2]["result"]["result"]["changes"]["modified"], 1,
        "{output}"
    );
    assert_eq!(
        output[3]["result"]["result"]["why"], "no_worktree",
        "{output}"
    );
    assert!(!output.to_string().contains("fixture-sentinel"));
}

fn capable() -> bool {
    use nix_peer::sys::socket::{getsockopt, sockopt::PeerPidfd};
    let (unix, _peer) = std::os::unix::net::UnixStream::pair().expect("local sockets");
    getsockopt(&unix, PeerPidfd)
        .is_ok_and(|fd| rustix::fs::fstatfs(fd).is_ok_and(|stats| stats.f_type == 0x5049_4446))
}

async fn ask(person: &mut super::Client, repository: &str, read: Read) -> Outcome {
    let id = person
        .call(Action::GitRead(Request {
            repository: repository.to_owned(),
            read,
        }))
        .await;
    person.done(id).await
}

fn denied(why: read::Refusal) -> Outcome {
    Outcome::GitRead {
        result: read::Outcome::Refused { why },
    }
}

async fn start(kit: &Kit, person: &mut super::Client, repository: &str) -> Uuid {
    let folder = kit.path(repository);
    std::fs::create_dir_all(&folder).expect("project");
    one_commit(&folder);
    let id = person
        .call(Action::Start {
            program: Program::ClaudeCode,
            provider_address: None,
            folder,
            mode: Mode::Ask,
            model: None,
            effort: None,
            speed: None,
            prompt: "Start the fixture".to_owned(),
            program_path: None,
            draft: None,
            attachments: Vec::new(),
            worktree: true,
            base: None,
        })
        .await;
    let Outcome::Started { thread, folder } = person.done(id).await else {
        panic!("the thread started")
    };
    std::fs::write(folder.join("a"), "fixture-sentinel\n").expect("a synthetic change");
    thread
}

async fn query(kit: &Kit, person: &mut super::Client, thread: Uuid, name: &str) -> Value {
    let output = kit.path(&format!("reads-{name}.json"));
    if output.exists() {
        std::fs::remove_file(&output).expect("clear prior response");
    }
    let id = person
        .call(Action::Send {
            thread,
            text: "Read Git summaries".to_owned(),
            attachments: Vec::new(),
            skills: Vec::new(),
        })
        .await;
    person.done(id).await;
    response(output).await
}

async fn response(path: PathBuf) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(bytes) = std::fs::read(&path) {
                return serde_json::from_slice(&bytes).expect("fixture wrote JSON");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("no fixture result at {}", path.display()))
}

async fn review(person: &mut super::Client, thread: Uuid, repository: &str) -> String {
    let id = person
        .call(Action::Git {
            thread,
            repository: repository.to_owned(),
            action: GitAction::Changes {
                scope: Scope::Uncommitted,
                ignore_whitespace: false,
            },
        })
        .await;
    let Outcome::Git {
        result: GitOutcome::Changes(change),
    } = person.done(id).await
    else {
        panic!("person can read Git changes")
    };
    change.snapshot.expect("complete person review")
}

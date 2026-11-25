//! The Git engine of the runner (TH116): the runner makes it at its start, keeps its records
//! in its own folder when the daemon has a home folder of its own, and answers a Git call of a
//! client in a task of its own.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use agent_threads::control::{Action, Outcome, RunState};
use agent_threads::drive::{Mode, Program};
use agent_threads::git::{GitAction, GitOutcome, GitRefusal};
use uuid::Uuid;

use super::runner::{Client, Kit, state_is};

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
mod agent_reads;

#[tokio::test]
async fn th116_a_git_call_is_answered_by_the_engine_of_the_runner() {
    let kit = Kit::new();
    let mut client = kit.client().await;
    let asked = client
        .call(Action::Git {
            thread: Uuid::now_v7(),
            repository: "app".to_owned(),
            action: GitAction::Status,
        })
        .await;
    assert_eq!(
        client.done(asked).await,
        Outcome::Git {
            result: GitOutcome::Refused {
                why: GitRefusal::NoWorktree
            }
        },
        "a thread with no worktree of the runner has no Git"
    );
    let records = kit.threads().join("git-records");
    assert!(
        records.is_dir(),
        "a daemon with a home folder of its own keeps the records in it, never in the place of the installed daemon"
    );
}

/// A made Claude Code that ends one turn for each prompt.
const CLAUDE_TURNS: &str = r#"#!/usr/bin/perl
$| = 1;
my $sid = "none";
for my $i (0 .. $#ARGV) { $sid = $ARGV[$i + 1] if $ARGV[$i] eq "--session-id"; }
print qq({"type":"system","subtype":"init","session_id":"$sid","model":"made","permissionMode":"default"}\n);
while (my $line = <STDIN>) {
    if ($line =~ /"type":"user"/) {
        print qq({"type":"system","subtype":"session_state_changed","state":"running"}\n);
        print qq({"type":"assistant","message":{"content":[{"type":"text","text":"a reply"}]}}\n);
        print qq({"type":"result","subtype":"success","is_error":false,"result":"done","terminal_reason":"completed"}\n);
        print qq({"type":"system","subtype":"session_state_changed","state":"idle"}\n);
    }
}
"#;

/// `git` in `folder`, with a name for its commits.
fn git(folder: &Path, args: &[&str]) {
    let done = Command::new("/usr/bin/git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .current_dir(folder)
        .output()
        .expect("git runs");
    assert!(done.status.success(), "git {args:?}");
}

/// A repository with one commit in `work`.
fn one_commit(work: &Path) {
    git(work, &["init", "-q"]);
    std::fs::write(work.join("a"), "a").expect("a file");
    git(work, &["add", "a"]);
    git(work, &["commit", "-q", "-m", "one"]);
}

/// The turns of the checkpoints of `thread`, once the one of its first turn is there (the
/// runner takes it in a task of its own), or as they are after 10 s.
async fn turns_of(client: &mut Client, thread: Uuid) -> Vec<u32> {
    let mut turns = Vec::new();
    for _ in 0..100 {
        let asked = client
            .call(Action::Git {
                thread,
                repository: "work".to_owned(),
                action: GitAction::Checkpoints,
            })
            .await;
        if let Outcome::Git {
            result: GitOutcome::Checkpoints { checkpoints },
        } = client.done(asked).await
        {
            turns = checkpoints
                .iter()
                .map(|checkpoint| checkpoint.turn)
                .collect();
        }
        if turns.len() > 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    turns
}

#[tokio::test]
async fn th116_a_thread_in_a_new_worktree_runs_there_and_each_turn_leaves_a_checkpoint() {
    let kit = Kit::with(CLAUDE_TURNS);
    let work = kit.path("work");
    one_commit(&work);
    let mut client = kit.client().await;
    let watch = client.call(Action::Watch).await;
    client.done(watch).await;
    let started = client
        .call(Action::Start {
            provider_address: None,
            program: Program::ClaudeCode,
            folder: work.clone(),
            mode: Mode::Ask,
            model: None,
            effort: None,
            speed: None,
            prompt: "Change it".to_owned(),
            program_path: None,
            draft: None,
            attachments: Vec::new(),
            worktree: true,
            base: None,
        })
        .await;
    let answer = client.done(started).await;
    let Outcome::Started { thread, folder } = answer else {
        panic!("the thread started: {answer:?}");
    };
    let real = |path: &Path| std::fs::canonicalize(path).expect("a real path");
    let worktrees = kit.path(".anima/worktrees");
    assert_eq!(
        real(&folder),
        real(&worktrees.join(thread.to_string()).join("work")),
        "the program runs in the worktree of the thread, not in the checkout of the person"
    );
    client.until(state_is(RunState::Idle)).await;
    assert_eq!(
        turns_of(&mut client, thread).await,
        [0, 1],
        "the worktree has its first checkpoint and the one of the turn"
    );
}

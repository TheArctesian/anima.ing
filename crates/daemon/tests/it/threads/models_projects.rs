//! What the copy of the T3 Code screens needs of the runner (TH104 to TH107): the models of a
//! program, plan mode, the updates read back, and the projects of this computer.

use agent_threads::control::{Action, Outcome, Project, ProjectChange, Refusal, RunState};
use agent_threads::drive::{Mode, ModelChoice, Program};
use uuid::Uuid;

use super::runner::{Client, Kit, state_is};

/// A made Claude Code that answers `initialize` with its models (and an account, which the
/// runner drops), then says one reply for each prompt.
const CLAUDE_MODELS: &str = r#"#!/usr/bin/perl
$| = 1;
my $sid = "none";
for my $i (0 .. $#ARGV) { $sid = $ARGV[$i + 1] if $ARGV[$i] eq "--session-id"; }
print qq({"type":"system","subtype":"init","session_id":"$sid","model":"made","permissionMode":"plan"}\n);
while (my $line = <STDIN>) {
    if ($line =~ /"subtype":"initialize"/ && $line =~ /"request_id":"([^"]+)"/) {
        print qq({"type":"control_response","response":{"subtype":"success","request_id":"$1","response":{"models":[{"value":"opus","displayName":"Opus","supportsEffort":true,"supportedEffortLevels":["low","high"]},{"value":"haiku","displayName":"Haiku"}],"account":{"email":"someone\@example.com"}}}}\n);
    } elsif ($line =~ /"type":"user"/) {
        print qq({"type":"system","subtype":"session_state_changed","state":"running"}\n);
        print qq({"type":"assistant","message":{"content":[{"type":"text","text":"a reply"}]}}\n);
        print qq({"type":"result","subtype":"success","is_error":false,"result":"done","terminal_reason":"completed","usage":{"input_tokens":3,"output_tokens":5}}\n);
        print qq({"type":"system","subtype":"session_state_changed","state":"idle"}\n);
    }
}
"#;

fn start(kit: &Kit, program: Program, mode: Mode) -> Action {
    Action::Start {
        program,
        folder: kit.path("work"),
        mode,
        model: None,
        effort: None,
        prompt: "Plan the change".to_owned(),
        program_path: None,
        draft: None,
        attachments: Vec::new(),
    }
}

/// The list of `program` in the answer to `models`.
async fn models_of(client: &mut Client, program: Program) -> Vec<ModelChoice> {
    let asked = client.call(Action::Models).await;
    let Outcome::Models { models } = client.done(asked).await else {
        panic!("models");
    };
    models
        .into_iter()
        .find(|row| row.program == program)
        .map_or_else(Vec::new, |row| row.models)
}

#[tokio::test]
async fn th104_the_models_of_each_program_are_there_before_any_thread() {
    let kit = Kit::with(CLAUDE_MODELS);
    let mut client = kit.client().await;
    let claude = models_of(&mut client, Program::ClaudeCode).await;
    assert_eq!(
        claude
            .iter()
            .map(|model| model.value.as_str())
            .collect::<Vec<_>>(),
        ["opus", "haiku"],
        "Claude Code names them in its answer to `initialize`, with no prompt"
    );
    let codex = models_of(&mut client, Program::Codex).await;
    assert_eq!(
        codex[0].value, "made-codex",
        "Codex names them on `model/list`"
    );
    assert_eq!(codex[0].default_effort.as_deref(), Some("low"));
}

async fn change_project(client: &mut Client, change: ProjectChange) -> Outcome {
    let id = client.call(Action::ChangeProject { change }).await;
    client.done(id).await
}

#[tokio::test]
async fn th104_th105_a_claude_start_in_plan_mode_names_its_models_and_codex_in_plan_mode_is_refused()
 {
    let kit = Kit::with(CLAUDE_MODELS);
    let mut client = kit.client().await;
    let refused = client.call(start(&kit, Program::Codex, Mode::Plan)).await;
    assert_eq!(
        client.done(refused).await,
        Outcome::Refused {
            why: Refusal::BadValue
        },
        "Codex has no plan mode that a start can set"
    );
    let watch = client.call(Action::Watch).await;
    client.done(watch).await;
    let started = client
        .call(start(&kit, Program::ClaudeCode, Mode::Plan))
        .await;
    let Outcome::Started { thread } = client.done(started).await else {
        panic!("the thread started");
    };
    client.until(state_is(RunState::Idle)).await;
    let claude = models_of(&mut client, Program::ClaudeCode).await;
    let values: Vec<&str> = claude.iter().map(|model| model.value.as_str()).collect();
    assert_eq!(values, ["opus", "haiku"]);
    assert_eq!(claude[0].efforts, ["low", "high"]);
    assert_eq!(claude[1].efforts, Vec::<String>::new());

    let list = client.call(Action::List).await;
    let Outcome::Threads { threads, .. } = client.done(list).await else {
        panic!("threads");
    };
    assert_eq!(threads[0].id, thread);
    assert_eq!(threads[0].mode, Mode::Plan);
}

#[tokio::test]
async fn th106_the_updates_of_a_thread_are_read_back_before_a_number() {
    let kit = Kit::with(CLAUDE_MODELS);
    let mut client = kit.client().await;
    let started = client
        .call(start(&kit, Program::ClaudeCode, Mode::FullAccess))
        .await;
    let Outcome::Started { thread } = client.done(started).await else {
        panic!("the thread started");
    };
    let watch = client.call(Action::Watch).await;
    client.done(watch).await;
    client.until(state_is(RunState::Idle)).await;

    let read = |before, most| Action::Earlier {
        thread,
        before,
        most,
    };
    let all = client.call(read(u64::MAX, 1000)).await;
    let Outcome::Earlier { updates } = client.done(all).await else {
        panic!("earlier");
    };
    assert!(updates.len() >= 3, "the prompt, the reply and the states");
    assert!(
        updates.windows(2).all(|pair| pair[0].seq < pair[1].seq),
        "oldest first"
    );
    let last = updates.last().expect("one").seq;
    let two = client.call(read(last, 2)).await;
    let Outcome::Earlier { updates: some } = client.done(two).await else {
        panic!("earlier");
    };
    assert_eq!(some.len(), 2);
    assert!(some.iter().all(|update| update.seq < last));
    assert_eq!(
        some[1].seq,
        updates[updates.len() - 2].seq,
        "the newest before the number"
    );

    let none = client.call(read(0, 10)).await;
    assert_eq!(
        client.done(none).await,
        Outcome::Earlier {
            updates: Vec::new()
        }
    );
    let unknown = client
        .call(Action::Earlier {
            thread: Uuid::now_v7(),
            before: 10,
            most: 10,
        })
        .await;
    assert_eq!(
        client.done(unknown).await,
        Outcome::Refused {
            why: Refusal::UnknownThread
        }
    );
}

#[tokio::test]
async fn th107_the_projects_are_kept_after_a_restart_and_a_folder_must_be_of_this_computer() {
    let mut kit = Kit::with(CLAUDE_MODELS);
    let mut client = kit.client().await;
    let work = kit.path("work");
    let real = std::fs::canonicalize(&work).expect("the folder");
    let added = change_project(
        &mut client,
        ProjectChange::Add {
            folder: work.clone(),
            name: None,
        },
    )
    .await;
    assert_eq!(
        added,
        Outcome::Projects {
            projects: vec![Project {
                folder: real.clone(),
                name: "work".to_owned()
            }]
        }
    );
    refuses_a_folder_not_of_this_computer_or_known(&kit, &mut client).await;
    change_project(
        &mut client,
        ProjectChange::Rename {
            folder: real.clone(),
            name: "The work".to_owned(),
        },
    )
    .await;
    drop(client);

    kit.end_runner();
    kit.start();
    let mut client = kit.client().await;
    let listed = client.call(Action::Projects).await;
    let Outcome::Projects { projects } = client.done(listed).await else {
        panic!("projects");
    };
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "The work");
    let removed = change_project(&mut client, ProjectChange::Remove { folder: real }).await;
    assert_eq!(
        removed,
        Outcome::Projects {
            projects: Vec::new()
        }
    );
}

/// A relative folder, a folder that is gone and a folder that is a project already.
async fn refuses_a_folder_not_of_this_computer_or_known(kit: &Kit, client: &mut Client) {
    let work = kit.path("work");
    for refused in [
        ProjectChange::Add {
            folder: "relative".into(),
            name: None,
        },
        ProjectChange::Add {
            folder: kit.path("gone"),
            name: None,
        },
        ProjectChange::Add {
            folder: work.clone(),
            name: None,
        },
    ] {
        assert!(
            matches!(
                change_project(client, refused.clone()).await,
                Outcome::Refused { .. }
            ),
            "{refused:?}"
        );
    }
}

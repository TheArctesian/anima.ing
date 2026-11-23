//! Each call reads back after it went as one frame: its fields sit beside the ID of the call, so
//! a field that the call has too would make the frame unreadable (TH109, TH111).

use std::path::PathBuf;

use uuid::Uuid;

use super::{Action, Call, ListingChange, Outcome, ProjectChange, Reply};
use crate::drive::{Answer, Mode, Program};
use crate::git::{GitAction, GitOutcome, Scope};

/// One of each call that reads, or that starts or drives a thread.
fn reads() -> Vec<Action> {
    let thread = Uuid::from_u128(7);
    let other = Uuid::from_u128(8);
    let text = || "words".to_owned();
    vec![
        Action::List,
        Action::Watch,
        Action::Start {
            program: Program::Codex,
            folder: PathBuf::from("/work"),
            mode: Mode::Ask,
            model: None,
            effort: None,
            prompt: text(),
            program_path: None,
            draft: None,
            attachments: Vec::new(),
        },
        Action::Send {
            thread,
            text: text(),
            attachments: vec![other],
        },
        Action::Answer {
            thread,
            request: other,
            answer: Answer::Allow,
        },
        Action::Interrupt { thread },
        Action::Stop { thread },
        Action::RaiseLimit,
        Action::Accounts,
        Action::Folder {
            folder: PathBuf::from("/work"),
        },
        Action::Folders {
            folder: PathBuf::from("/work"),
            hidden: true,
        },
        Action::Listings,
    ]
}

/// One of each call that changes a thread or how it is listed.
fn changes() -> Vec<Action> {
    let thread = Uuid::from_u128(7);
    let other = Uuid::from_u128(8);
    let text = || "words".to_owned();
    vec![
        Action::ChangeListing {
            thread,
            change: ListingChange::Remove,
        },
        Action::Models,
        Action::Earlier {
            thread,
            before: 9,
            most: 3,
        },
        Action::Projects,
        Action::ChangeProject {
            change: ProjectChange::Remove {
                folder: PathBuf::from("/work"),
            },
        },
        Action::Files {
            thread,
            query: text(),
            most: 5,
        },
        Action::Queue {
            thread,
            text: text(),
        },
        Action::Requeue {
            thread,
            queued: other,
            text: text(),
        },
        Action::Unqueue {
            thread,
            queued: other,
        },
        Action::SendQueued {
            thread,
            queued: other,
        },
        Action::Set {
            thread,
            model: Some("m".to_owned()),
            effort: None,
            mode: Some(Mode::Ask),
        },
    ]
}

/// One of each call that sends a file for a prompt (TH111).
fn files() -> Vec<Action> {
    let thread = Uuid::from_u128(7);
    let other = Uuid::from_u128(8);
    vec![
        Action::Attach {
            thread,
            name: "a.png".to_owned(),
            mime: "image/png".to_owned(),
            size: 3,
            bytes: "AAAA".to_owned(),
            draft: false,
        },
        Action::AttachMore {
            thread,
            attachment: other,
            bytes: "AAAA".to_owned(),
        },
        Action::Unattach {
            thread,
            attachment: other,
        },
        Action::Git {
            thread,
            repository: "app".to_owned(),
            action: GitAction::Changes {
                scope: Scope::Turn { turn: 3 },
                ignore_whitespace: true,
            },
        },
        Action::Git {
            thread,
            repository: "app".to_owned(),
            action: GitAction::Commit {
                message: "words".to_owned(),
                reviewed: "a1".to_owned(),
                files: Some(vec!["src/a.rs".to_owned()]),
                new_branch: None,
            },
        },
    ]
}

/// The number of each kind of call. The match names each kind, so a new kind does not build
/// until it has a number here; the test then fails until it is in a list above.
const fn kind(action: &Action) -> usize {
    match action {
        Action::List => 0,
        Action::Watch => 1,
        Action::Start { .. } => 2,
        Action::Send { .. } => 3,
        Action::Answer { .. } => 4,
        Action::Interrupt { .. } => 5,
        Action::Stop { .. } => 6,
        Action::RaiseLimit => 7,
        Action::Accounts => 8,
        Action::Folder { .. } => 9,
        Action::Listings => 10,
        Action::ChangeListing { .. } => 11,
        Action::Models => 12,
        Action::Earlier { .. } => 13,
        Action::Projects => 14,
        Action::ChangeProject { .. } => 15,
        Action::Files { .. } => 16,
        Action::Queue { .. } => 17,
        Action::Requeue { .. } => 18,
        Action::Unqueue { .. } => 19,
        Action::SendQueued { .. } => 20,
        Action::Set { .. } => 21,
        Action::Attach { .. } => 22,
        Action::AttachMore { .. } => 23,
        Action::Unattach { .. } => 24,
        Action::Folders { .. } => 25,
        Action::Git { .. } => 26,
    }
}

const KINDS: usize = 27;

#[test]
fn th109_each_call_reads_back_from_its_frame() {
    let mut named = [false; KINDS];
    for action in reads().into_iter().chain(changes()).chain(files()) {
        named[kind(&action)] = true;
        let call = Call {
            id: 42,
            action: action.clone(),
        };
        let frame = serde_json::to_string(&call).expect("a frame");
        let read: Call =
            serde_json::from_str(&frame).unwrap_or_else(|error| panic!("{frame}: {error}"));
        assert_eq!(read, call, "{frame}");
    }
    assert!(
        named.iter().all(|named| *named),
        "each kind of call is in a list"
    );
}

#[test]
fn th116_a_git_answer_reads_back_from_its_frame() {
    let done = Reply::Done {
        id: 42,
        result: Outcome::Git {
            result: GitOutcome::Reverted {
                turn: 2,
                undo: "b2".to_owned(),
            },
        },
    };
    let frame = serde_json::to_string(&done).expect("a frame");
    let read: Reply =
        serde_json::from_str(&frame).unwrap_or_else(|error| panic!("{frame}: {error}"));
    assert_eq!(read, done, "{frame}");
}

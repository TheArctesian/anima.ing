//! The pure rules of a copy and the checks of a pull (K11 Sections 7 and 11).

use chrono::{DateTime, Utc};
use daemon_core::{
    model_work::names::Actor,
    wire::{Cleanup, CopyOutcome, Limits, ModelOutcome, Progress, WorkKind, WorkResult, WorkType},
};
use uuid::Uuid;

use super::{
    CopyState,
    order::{self, unpullable},
    rules::{Removal, Repull, removal, repull, status},
};
use crate::{
    fleet::compute::library::{Format, PullSource, Variant, VariantFile},
    ids::{CopyId, DeviceId, JobId},
    work::jobs::{Job, NewJob, State, Target},
};

fn at() -> DateTime<Utc> {
    "2026-11-23T10:00:00Z".parse().unwrap()
}

fn job(work_type: WorkType, state: State, attempt: u32) -> Job {
    Job {
        id: JobId::from(Uuid::now_v7()),
        asked: NewJob {
            work_type,
            kind: WorkKind::Model,
            os: None,
            architecture: None,
            project: None,
            target: Target::Model { actor: Actor::User },
            parameters: serde_json::json!({}),
            secrets: Vec::new(),
            limits: Limits {
                seconds: 1,
                idle_seconds: 1,
                memory_bytes: 1,
                disk_bytes: 1,
                orphan_seconds: 1,
            },
            machine: None,
        },
        state,
        attempt,
        lost: 0,
        holder: None,
        last_device: None,
        refused_by: Vec::new(),
        lease_ends_at: None,
        claimed_at: None,
        stop: None,
        reason: None,
        result: None,
        log_end: 0,
        progress: None,
        created_at: at(),
        changed_at: at(),
        ended_at: None,
    }
}

fn pull(state: State) -> Job {
    job(
        WorkType::ModelPull,
        state,
        u32::from(state != State::Queued),
    )
}

fn delete(state: State) -> Job {
    job(WorkType::CopyDelete, state, 1)
}

fn state_of(pull: &Job, delete: Option<&Job>) -> Option<CopyState> {
    status(Some(pull), delete).map(|status| status.state)
}

#[test]
fn the_state_of_a_copy_follows_its_pull() {
    assert_eq!(
        state_of(&pull(State::Queued), None),
        Some(CopyState::Queued)
    );
    for held in [
        State::Claimed,
        State::Preparing,
        State::Running,
        State::Finishing,
    ] {
        assert_eq!(state_of(&pull(held), None), Some(CopyState::Downloading));
    }
    assert_eq!(
        state_of(&pull(State::Done), None),
        Some(CopyState::Downloaded)
    );
    let mut failed = pull(State::Failed);
    failed.reason = Some("checksum".to_owned());
    let found = status(Some(&failed), None).unwrap();
    assert_eq!(found.state, CopyState::Failed);
    assert_eq!(found.reason.as_deref(), Some("checksum"));
    assert_eq!(
        state_of(&pull(State::Stopped), None),
        Some(CopyState::Failed)
    );
}

#[test]
fn pull_007_only_a_running_pull_shows_its_progress() {
    let progress = Progress {
        bytes: 5,
        total_bytes: 10,
        bytes_per_second: 1,
    };
    let mut running = pull(State::Running);
    running.progress = Some(progress);
    assert_eq!(
        status(Some(&running), None).unwrap().progress,
        Some(progress)
    );
    let mut done = pull(State::Done);
    done.progress = Some(progress);
    assert_eq!(status(Some(&done), None).unwrap().progress, None);
}

#[test]
fn pull_011_a_pull_that_succeeds_takes_the_state_that_the_machine_reports() {
    let mut done = pull(State::Done);
    done.result = Some(WorkResult {
        health: None,
        model: Some(ModelOutcome::Copy(CopyOutcome {
            copy: Uuid::now_v7(),
            copy_state: "validated".to_owned(),
            bytes: 10,
            files: 1,
            file_hashes: Vec::new(),
        })),
        outcome: "succeeded".to_owned(),
        cause: None,
        seconds: 1,
        cleanup: Cleanup {
            isolation: "done".to_owned(),
            checkout: "done".to_owned(),
            secrets: "done".to_owned(),
            keychain: "done".to_owned(),
        },
    });
    assert_eq!(state_of(&done, None), Some(CopyState::Validated));
}

#[test]
fn pull_009_a_removal_hides_the_copy_when_it_is_done() {
    let downloaded = pull(State::Done);
    assert_eq!(
        state_of(&downloaded, Some(&delete(State::Queued))),
        Some(CopyState::Removing)
    );
    assert_eq!(state_of(&downloaded, Some(&delete(State::Done))), None);
    let mut failed = delete(State::Failed);
    failed.reason = Some("delete_failed".to_owned());
    let kept = status(Some(&downloaded), Some(&failed)).unwrap();
    assert_eq!(kept.state, CopyState::Downloaded);
    assert_eq!(kept.reason.as_deref(), Some("delete_failed"));
}

#[test]
fn pull_009_a_removal_stops_a_pull_and_deletes_what_a_machine_may_hold() {
    assert_eq!(removal(&pull(State::Queued), None), Removal::StopQueued);
    let mut again = pull(State::Queued);
    again.attempt = 2;
    assert_eq!(removal(&again, None), Removal::StopAndDelete);
    assert_eq!(removal(&pull(State::Running), None), Removal::StopAndDelete);
    assert_eq!(removal(&pull(State::Done), None), Removal::Delete);
    assert_eq!(removal(&pull(State::Failed), None), Removal::Delete);
    let stopped_in_queue = job(WorkType::ModelPull, State::Stopped, 0);
    assert_eq!(removal(&stopped_in_queue, None), Removal::Forget);
    let done = pull(State::Done);
    assert_eq!(
        removal(&done, Some(&delete(State::Running))),
        Removal::Nothing
    );
    assert_eq!(removal(&done, Some(&delete(State::Done))), Removal::Forget);
    assert_eq!(
        removal(&done, Some(&delete(State::Failed))),
        Removal::Delete
    );
}

#[test]
fn a_failed_copy_is_pulled_again_and_a_copy_that_is_there_is_not() {
    assert_eq!(repull(Some(&pull(State::Failed)), None), Repull::Again);
    assert_eq!(
        repull(Some(&pull(State::Done)), None),
        Repull::Taken(CopyState::Downloaded)
    );
    assert_eq!(
        repull(Some(&pull(State::Running)), None),
        Repull::Taken(CopyState::Downloading)
    );
    let done = pull(State::Done);
    assert_eq!(
        repull(Some(&done), Some(&delete(State::Done))),
        Repull::Replace
    );
    assert_eq!(
        repull(Some(&done), Some(&delete(State::Queued))),
        Repull::Taken(CopyState::Removing)
    );
}

fn file(path: &str) -> VariantFile {
    VariantFile {
        path: path.to_owned(),
        size_bytes: 5,
        sha256: "b".repeat(64),
    }
}

fn source(files: Vec<VariantFile>) -> PullSource {
    PullSource {
        variant_id: Uuid::now_v7(),
        model: "qwen3".to_owned(),
        variant: Variant {
            tag: "8b-q4_k_m".to_owned(),
            format: Format::Gguf,
            quantisation: "Q4_K_M".to_owned(),
            repository: "unsloth/Qwen3-8B-GGUF".to_owned(),
            revision: "a".repeat(40),
            files,
        },
    }
}

#[test]
fn a_pull_is_refused_at_the_hub_for_files_that_a_machine_refuses() {
    assert_eq!(
        unpullable(&source(vec![file("Qwen3-8B-Q4_K_M.gguf")])),
        None
    );
    assert!(unpullable(&source(vec![file("model.bin")])).is_some());
    assert!(unpullable(&source(vec![file("../x.gguf")])).is_some());
    assert!(unpullable(&source(Vec::new())).is_some());
    // R-408: two files that flatten to one name in the folder of the copy.
    assert!(unpullable(&source(vec![file("a/Q4.gguf"), file("b/q4.gguf")])).is_some());
    let mut bad = source(vec![file("Q4.gguf")]);
    bad.variant.revision = "main".to_owned();
    assert!(unpullable(&bad).is_some());
}

#[test]
fn the_pull_item_has_the_members_of_k11_and_the_machine() {
    let copy = CopyId::from(Uuid::now_v7());
    let device = DeviceId::from(Uuid::now_v7());
    let wanted = order::pull(copy, &source(vec![file("Q4.gguf")]), (device, Actor::Agent));
    assert_eq!(wanted.work_type, WorkType::ModelPull);
    assert_eq!(wanted.machine, Some(device));
    assert_eq!(
        wanted.target,
        Target::Model {
            actor: Actor::Agent
        }
    );
    assert_eq!(wanted.limits.disk_bytes, 5);
    let parameters = &wanted.parameters;
    let mut members: Vec<&str> = parameters
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    members.sort_unstable();
    assert_eq!(
        members,
        ["copy", "files", "format", "gated", "source", "variant"]
    );
    assert_eq!(parameters["source"]["organisation"], "unsloth");
    assert_eq!(parameters["source"]["repository"], "Qwen3-8B-GGUF");
    assert_eq!(parameters["gated"], false);
    assert_eq!(wanted.secrets.len(), 0);
}

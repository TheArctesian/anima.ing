//! The pure rules of a copy: its state from the jobs of its pull and its removal, what a
//! removal does, and whether a pull may start (K11 Section 11, PULL-009, PULL-011).

use crate::work::jobs::{Job, State};

use super::model::{CopyState, Status};

/// The reason of a pull that ended with no cause that the hub keeps.
const NO_CAUSE: &str = "unknown";

/// The state of a copy from the job of its pull and the job of its removal. `None` for a copy
/// whose removal is done: the machine no longer has it.
#[must_use]
pub fn status(pull: Option<&Job>, delete: Option<&Job>) -> Option<Status> {
    if let Some(delete) = delete {
        match delete.state {
            State::Done => return None,
            state if !state.is_final() => {
                return Some(plain(CopyState::Removing, None));
            }
            _ => {}
        }
    }
    let mut status = pull.map_or_else(
        || plain(CopyState::Failed, Some(NO_CAUSE.to_owned())),
        of_pull,
    );
    // A removal that failed leaves the copy, and its cause is the reason to show.
    if let Some(reason) = delete.and_then(|delete| delete.reason.clone()) {
        status.reason = Some(reason);
    }
    Some(status)
}

const fn plain(state: CopyState, reason: Option<String>) -> Status {
    Status {
        state,
        progress: None,
        reason,
    }
}

fn of_pull(pull: &Job) -> Status {
    match pull.state {
        State::Queued => plain(CopyState::Queued, None),
        State::Running => Status {
            progress: pull.progress,
            ..plain(CopyState::Downloading, None)
        },
        State::Claimed | State::Preparing | State::Finishing => plain(CopyState::Downloading, None),
        State::Done => {
            let reported = pull
                .result
                .as_ref()
                .and_then(|result| result.copy())
                .and_then(|model| CopyState::from_reported(&model.copy_state));
            plain(reported.unwrap_or(CopyState::Downloaded), None)
        }
        State::Failed | State::Stopped => plain(
            CopyState::Failed,
            Some(pull.reason.clone().unwrap_or_else(|| NO_CAUSE.to_owned())),
        ),
    }
}

/// What the removal of a copy does (PULL-009).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    /// A removal is already asked for: nothing more.
    Nothing,
    /// No machine has a byte of it: the record goes.
    Forget,
    /// The pull waits in the queue: stop it, then forget the copy if it stopped there.
    StopQueued,
    /// A machine holds the pull: stop it, and delete what it wrote.
    StopAndDelete,
    /// The pull ended on a machine: delete the copy there.
    Delete,
}

#[must_use]
pub fn removal(pull: &Job, delete: Option<&Job>) -> Removal {
    match delete.map(|delete| delete.state) {
        Some(State::Done) => return Removal::Forget,
        Some(state) if !state.is_final() => return Removal::Nothing,
        _ => {}
    }
    match pull.state {
        State::Queued if pull.attempt == 0 => Removal::StopQueued,
        state if state.is_held() || state == State::Queued => Removal::StopAndDelete,
        _ if pull.attempt == 0 => Removal::Forget,
        _ => Removal::Delete,
    }
}

/// Whether a pull of a `model:tag` may start on a machine that has a record of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repull {
    /// The record is of a removed copy: it goes, and a new copy starts.
    Replace,
    /// The copy failed: its pull starts again with the same copy (K11 Section 11).
    Again,
    /// The machine has the copy, pulls it or removes it.
    Taken(CopyState),
}

#[must_use]
pub fn repull(pull: Option<&Job>, delete: Option<&Job>) -> Repull {
    match status(pull, delete) {
        None => Repull::Replace,
        Some(status) if status.state == CopyState::Failed => Repull::Again,
        Some(status) => Repull::Taken(status.state),
    }
}

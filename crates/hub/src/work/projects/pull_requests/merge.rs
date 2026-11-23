//! The merge rule (K-E1 Section 13.3; PRJ-033, PRO-013). Pure.
//! A mergeability GitHub has not worked out, or merge methods the hub cannot see, refuse.
//! A pull request from a fork is a trust decision by a person: it merges only alone, only when the caller typed its head repository, and never for an AI agent.

use provider_github::live::pulls::{PullDetail, PullState};

use super::{
    bulk::MergeOutcome,
    views::{MergeMethod, MergeState, merge_state},
};
use crate::api_errors::ErrorCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Merging<'a> {
    Alone {
        allow_failing_checks: bool,
        fork_confirmed: Option<&'a str>,
    },
    InBulk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ask<'a> {
    pub method: MergeMethod,
    pub expected_head: &'a str,
    /// The request declares an AI agent (LOG-016, LOG-017).
    pub agent: bool,
    pub merging: Merging<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Merge,
    AlreadyMerged,
    Refused(Refusal),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Closed,
    Draft,
    Conflict,
    Blocked,
    Behind,
    ChecksFailing,
    NotReady,
    HeadMoved,
    NoMethods,
    MethodRefused,
    ForkInBulk,
    ForkUnconfirmed,
    ForkByAgent,
}

#[must_use]
pub fn decide(pull: &PullDetail, methods: &[MergeMethod], asked: &Ask<'_>) -> Decision {
    if pull.pull.state == PullState::Merged {
        return Decision::AlreadyMerged;
    }
    refusal(pull, methods, asked).map_or(Decision::Merge, Decision::Refused)
}

fn refusal(pull: &PullDetail, methods: &[MergeMethod], asked: &Ask<'_>) -> Option<Refusal> {
    let read = &pull.pull;
    if read.from_fork
        && let Some(refused) = fork(read.head_repository.as_deref(), asked)
    {
        return Some(refused);
    }
    if read.state != PullState::Open {
        return Some(Refusal::Closed);
    }
    if !read.head_sha.eq_ignore_ascii_case(asked.expected_head) {
        return Some(Refusal::HeadMoved);
    }
    if read.draft {
        return Some(Refusal::Draft);
    }
    if methods.is_empty() {
        return Some(Refusal::NoMethods);
    }
    if !methods.contains(&asked.method) {
        return Some(Refusal::MethodRefused);
    }
    mergeability(pull, asked.merging)
}

/// A deleted head repository cannot be typed, so it never merges (PRO-013).
fn fork(head_repository: Option<&str>, asked: &Ask<'_>) -> Option<Refusal> {
    match asked.merging {
        Merging::InBulk => Some(Refusal::ForkInBulk),
        _ if asked.agent => Some(Refusal::ForkByAgent),
        Merging::Alone { fork_confirmed, .. } => match (fork_confirmed, head_repository) {
            (Some(typed), Some(head)) if typed == head => None,
            _ => Some(Refusal::ForkUnconfirmed),
        },
    }
}

/// Only CLEAN merges in bulk, and UNSTABLE only alone with `allowFailingChecks`; the hub never
/// bypasses a required check, so BLOCKED does not merge.
fn mergeability(pull: &PullDetail, merging: Merging<'_>) -> Option<Refusal> {
    let unstable_allowed = matches!(
        merging,
        Merging::Alone {
            allow_failing_checks: true,
            ..
        }
    );
    let refused = match merge_state(pull.mergeable_state) {
        MergeState::Clean => None,
        MergeState::Unstable if unstable_allowed => None,
        MergeState::Unstable => Some(Refusal::ChecksFailing),
        MergeState::Conflict => Some(Refusal::Conflict),
        MergeState::Blocked => Some(Refusal::Blocked),
        MergeState::Behind => Some(Refusal::Behind),
        MergeState::Draft => Some(Refusal::Draft),
        MergeState::Unknown => Some(Refusal::NotReady),
    };
    refused.or(match pull.mergeable {
        Some(true) => None,
        Some(false) => Some(Refusal::Conflict),
        None => Some(Refusal::NotReady),
    })
}

impl Refusal {
    /// The fixed words: never a reply of GitHub or text from the pull request (LOG-007).
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Self::Closed => "It cannot merge now: it is closed.",
            Self::Draft => "It cannot merge now: it is a draft.",
            Self::Conflict => "It cannot merge now: it has a conflict.",
            Self::Blocked => "It cannot merge now: a required check or review is missing.",
            Self::Behind => "It cannot merge now: the base branch moved. Update the branch.",
            Self::ChecksFailing => {
                "It cannot merge now: a check failed or still runs. Merge it alone with \
                 allowFailingChecks."
            }
            Self::NotReady => "GitHub has not worked out whether it can merge. Try again.",
            Self::HeadMoved => "The pull request changed since you looked. Read it again.",
            Self::NoMethods => {
                "The hub cannot see which merge methods the repository allows. Give the GitHub \
                 App Contents write."
            }
            Self::MethodRefused => "The repository does not allow that merge method.",
            Self::ForkInBulk => {
                "It is from a fork: merge it alone, after you type its head repository (PRO-013)."
            }
            Self::ForkUnconfirmed => {
                "It is from a fork: set forkConfirmed to its head repository to merge it \
                 (PRO-013)."
            }
            Self::ForkByAgent => {
                "An AI agent cannot merge a pull request from a fork. The operator merges it \
                 (PRO-013)."
            }
        }
    }

    #[must_use]
    pub const fn code(self) -> ErrorCode {
        match self {
            Self::ForkInBulk | Self::ForkUnconfirmed | Self::ForkByAgent => ErrorCode::Forbidden,
            _ => ErrorCode::Conflict,
        }
    }

    #[must_use]
    pub const fn outcome(self) -> MergeOutcome {
        match self {
            Self::Closed
            | Self::Draft
            | Self::Conflict
            | Self::Blocked
            | Self::Behind
            | Self::ChecksFailing => MergeOutcome::NotMergeable,
            Self::NotReady => MergeOutcome::NotReady,
            Self::HeadMoved => MergeOutcome::HeadMoved,
            Self::NoMethods | Self::MethodRefused => MergeOutcome::MethodRefused,
            Self::ForkInBulk | Self::ForkUnconfirmed | Self::ForkByAgent => {
                MergeOutcome::ForkRefused
            }
        }
    }
}

//! The `parameters` of each container type, named as the contract names them (K10 6.4.6 to 6.4.11).

use crate::{
    Refusal,
    forms::{is_lower_hex, is_project_name, is_uuid},
    strict::{Members, Value},
};

const UNKNOWN: Refusal = Refusal::MemberUnknown;

/// There is no `kill`, no `pause` and no `remove` (Section 6.4.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Start,
    Stop,
    Restart,
}

/// What `adopted_apply` does to one project. There is no `down` (Section 6.4.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdoptedAction {
    Up,
    Stop,
    Restart,
}

/// One member of `what` of `docker_prune`. There is no `volumes` (Section 6.4.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pruned {
    Containers,
    Images,
    Networks,
}

/// The parameters of `application_apply` (Section 6.4.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Apply {
    pub application: String,
    pub file_sha256: String,
    pub environment_sha256: Option<String>,
    pub pull: bool,
    pub keep: bool,
}

/// The parameters of `application_remove` (Section 6.4.8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removal {
    pub application: String,
    pub volumes: bool,
    pub images: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Work {
    Container {
        container: String,
        action: Action,
    },
    Apply(Apply),
    Adopted {
        project: String,
        action: AdoptedAction,
        pull: bool,
    },
    Remove(Removal),
    Back {
        application: String,
    },
    Prune {
        what: Vec<Pruned>,
    },
    Follow {
        container: String,
        tail: u64,
    },
}

impl Work {
    /// The maximum of `limits.seconds` of the type (Section 7).
    #[must_use]
    pub const fn most_seconds(&self) -> u64 {
        match self {
            Self::Apply(_) | Self::Adopted { .. } => 1800,
            Self::Remove(_) | Self::Back { .. } | Self::Prune { .. } => 600,
            Self::Container { .. } => 120,
            Self::Follow { .. } => 900,
        }
    }

    #[must_use]
    pub const fn changes_docker(&self) -> bool {
        !matches!(self, Self::Follow { .. })
    }
}

pub(super) fn is_container_type(kind: &str) -> bool {
    matches!(
        kind,
        "container_action"
            | "application_apply"
            | "adopted_apply"
            | "application_remove"
            | "application_back"
            | "docker_prune"
            | "container_follow"
    )
}

/// The full identifier of a container: a shorter value or a name is refused.
fn container(members: &mut Members) -> Option<String> {
    members.text("container").filter(|id| is_lower_hex(id, 64))
}

fn application(members: &mut Members) -> Option<String> {
    members.text("application").filter(|id| is_uuid(id))
}

fn action(members: &mut Members) -> Option<Work> {
    let container = container(members)?;
    let action = match members.text("action")?.as_str() {
        "start" => Action::Start,
        "stop" => Action::Stop,
        "restart" => Action::Restart,
        _ => return None,
    };
    Some(Work::Container { container, action })
}

fn apply(members: &mut Members) -> Option<Work> {
    let application = application(members)?;
    let file_sha256 = members
        .text("fileSha256")
        .filter(|hash| is_lower_hex(hash, 64))?;
    let environment_sha256 = match members.take("environmentSha256")? {
        Value::Null => None,
        Value::Text(hash) if is_lower_hex(&hash, 64) => Some(hash),
        _ => return None,
    };
    let pull = members.truth("pull")?;
    let keep = match members.whole("keep")? {
        0 => false,
        1 => true,
        _ => return None,
    };
    Some(Work::Apply(Apply {
        application,
        file_sha256,
        environment_sha256,
        pull,
        keep,
    }))
}

fn adopted(members: &mut Members) -> Option<Work> {
    let project = members
        .text("project")
        .filter(|name| is_project_name(name))?;
    let action = match members.text("action")?.as_str() {
        "up" => AdoptedAction::Up,
        "stop" => AdoptedAction::Stop,
        "restart" => AdoptedAction::Restart,
        _ => return None,
    };
    let pull = members.truth("pull")?;
    (!pull || action == AdoptedAction::Up).then_some(Work::Adopted {
        project,
        action,
        pull,
    })
}

fn removal(members: &mut Members) -> Option<Work> {
    Some(Work::Remove(Removal {
        application: application(members)?,
        volumes: members.truth("volumes")?,
        images: members.truth("images")?,
    }))
}

fn prune(members: &mut Members) -> Option<Work> {
    let mut what = Vec::new();
    for name in members.texts("what")? {
        let member = match name.as_str() {
            "containers" => Pruned::Containers,
            "images" => Pruned::Images,
            "networks" => Pruned::Networks,
            _ => return None,
        };
        if what.contains(&member) {
            return None;
        }
        what.push(member);
    }
    (!what.is_empty()).then_some(Work::Prune { what })
}

fn follow(members: &mut Members) -> Option<Work> {
    let container = container(members)?;
    let tail = members.whole("tail").filter(|tail| *tail <= 1000)?;
    Some(Work::Follow { container, tail })
}

pub(super) fn read(kind: &str, parameters: Value) -> Result<Work, Refusal> {
    let mut members = Members::of(parameters).ok_or(UNKNOWN)?;
    let work = match kind {
        "container_action" => action(&mut members),
        "application_apply" => apply(&mut members),
        "adopted_apply" => adopted(&mut members),
        "application_remove" => removal(&mut members),
        "application_back" => {
            application(&mut members).map(|application| Work::Back { application })
        }
        "docker_prune" => prune(&mut members),
        "container_follow" => follow(&mut members),
        _ => None,
    };
    work.filter(|_| members.is_empty()).ok_or(UNKNOWN)
}

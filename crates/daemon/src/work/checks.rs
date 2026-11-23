//! Checks a claimed item before its start; the machine, not the hub, decides (K10 Sections 3 and 5).

use daemon_core::wire::{WorkJob, WorkKind};
use serde_json::{Map, Value};
use uuid::Uuid;

use super::{
    permitted::Repositories,
    refusal::Cause,
    shape,
    types::{self, Version},
};

type Object = Map<String, Value>;

#[derive(Clone, Copy, Debug)]
pub struct Machine<'a> {
    /// The switch at the machine (USE-004).
    pub switch_on: bool,
    /// The device policy permits work now (USE-009).
    pub policy_permits: bool,
    /// The kinds that the owner permits at this machine (PRO-024).
    pub kinds: &'a [WorkKind],
    pub isolation: &'a [WorkKind],
    /// The last clean-up did not delete each thing (K10 Section 9).
    pub unclean: bool,
    pub repositories: &'a Repositories,
    /// The lowest version of the runner program that this daemon accepts (SF-014).
    pub lowest_runner: Version,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Checked {
    Start(Box<WorkJob>),
    Refuse {
        id: Uuid,
        attempt: u32,
        cause: Cause,
    },
    /// The answer names no item and no attempt, so no report could name it.
    Drop,
}

fn kind_of(item: &Object) -> Option<WorkKind> {
    match shape::text(item, "kind")? {
        "container" => Some(WorkKind::Container),
        "native" => Some(WorkKind::Native),
        _ => None,
    }
}

fn typed(item: &Object, work_type: &str, lowest: Version) -> Result<(), Cause> {
    let secrets = shape::purposes(item).ok_or(Cause::MemberUnknown)?;
    let parameters = item
        .get("parameters")
        .and_then(Value::as_object)
        .ok_or(Cause::MemberUnknown)?;
    if !secrets
        .iter()
        .all(|purpose| types::purposes_of(work_type).contains(purpose))
    {
        return Err(Cause::SecretUnknown);
    }
    if work_type == "build" {
        return types::build_holds(parameters, &secrets)
            .then_some(())
            .ok_or(Cause::MemberUnknown);
    }
    let version = types::runner_version(parameters).ok_or(Cause::MemberUnknown)?;
    (version >= lowest).then_some(()).ok_or(Cause::RunnerTooOld)
}

/// The first true cause of K10 Section 5, in table order except `runner_too_old`, read with the type.
fn cause(item: &Object, machine: &Machine<'_>) -> Result<WorkJob, Cause> {
    let work_type = shape::text(item, "type")
        .filter(|name| ["build", "runner"].contains(name))
        .ok_or(Cause::TypeUnknown)?;
    let kind = kind_of(item)
        .filter(|kind| machine.kinds.contains(kind))
        .ok_or(Cause::KindNotPermitted)?;
    if !shape::holds(item) {
        return Err(Cause::MemberUnknown);
    }
    typed(item, work_type, machine.lowest_runner)?;
    let job: WorkJob =
        serde_json::from_value(Value::Object(item.clone())).map_err(|_| Cause::MemberUnknown)?;
    let states = [
        (machine.switch_on, Cause::SwitchOff),
        (machine.policy_permits, Cause::Policy),
        (machine.isolation.contains(&kind), Cause::NoIsolation),
        (!machine.unclean, Cause::Unclean),
        (
            machine.repositories.permits(&job.repository),
            Cause::RepositoryNotPermitted,
        ),
    ];
    match states.into_iter().find(|(holds, _)| !holds) {
        Some((_, cause)) => Err(cause),
        None => Ok(job),
    }
}

#[must_use]
pub fn check(answer: &Value, machine: &Machine<'_>) -> Checked {
    let Some(item) = answer.as_object() else {
        return Checked::Drop;
    };
    let id = shape::text(item, "id").and_then(|id| Uuid::parse_str(id).ok());
    let attempt = item
        .get("attempt")
        .and_then(Value::as_u64)
        .and_then(|attempt| u32::try_from(attempt).ok())
        .filter(|attempt| *attempt >= 1);
    let (Some(id), Some(attempt)) = (id, attempt) else {
        return Checked::Drop;
    };
    match cause(item, machine) {
        Ok(job) => Checked::Start(Box::new(job)),
        Err(cause) => Checked::Refuse { id, attempt, cause },
    }
}

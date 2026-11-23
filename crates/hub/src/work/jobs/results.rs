//! The result of an attempt as the hub keeps it (K10 Section 7): fixed codes and numbers, never text from the job or the machine.
//! A machine, or a person with its daemon token, could otherwise put text of its choice where a screen or an MCP tool shows it.

use daemon_core::model_work::cause::Cause;
use daemon_core::wire::{Cleanup, CopyOutcome, WorkKind, WorkResult};

/// The outcomes of K10 Section 7.
const OUTCOMES: [&str; 5] = ["succeeded", "failed", "idle", "stopped", "refused"];

/// The causes of K10 Section 5 (a refusal) and of Section 7 (an end).
const CAUSES: [&str; 18] = [
    "type_unknown",
    "kind_not_permitted",
    "member_unknown",
    "secret_unknown",
    "switch_off",
    "policy",
    "no_isolation",
    "unclean",
    "repository_not_permitted",
    "runner_too_old",
    "step_failed",
    "limit_seconds",
    "limit_memory",
    "limit_disk",
    "program_mismatch",
    "secret_failed",
    "isolation_failed",
    "orphan",
];

/// What the hub stores for a cause that it does not know (K10 Section 7).
pub(super) const UNKNOWN_CAUSE: &str = "failed";

const CLEANED: [&str; 3] = ["deleted", "failed", "none"];

/// The states of a copy (K11 Section 11).
const COPY_STATES: [&str; 5] = [
    "downloading",
    "downloaded",
    "validating",
    "validated",
    "failed",
];

/// The result with only codes of the fixed lists: a cause that the hub does not know becomes
/// `failed`. A model item also has the causes of K11 Section 5 and the member `model`; each
/// other kind has no `model`. `None` when the outcome, a member of `cleanup` or the state of
/// the copy is not of its list: the report is `MALFORMED`.
pub(super) fn checked(result: &WorkResult, kind: WorkKind) -> Option<WorkResult> {
    let Cleanup {
        isolation,
        checkout,
        secrets,
        keychain,
    } = &result.cleanup;
    let cleaned = [isolation, checkout, secrets, keychain]
        .iter()
        .all(|member| CLEANED.contains(&member.as_str()));
    let model = kind == WorkKind::Model;
    let copy_fits = match &result.model {
        None => true,
        Some(CopyOutcome { copy_state, .. }) => model && COPY_STATES.contains(&copy_state.as_str()),
    };
    if !cleaned || !copy_fits || !OUTCOMES.contains(&result.outcome.as_str()) {
        return None;
    }
    let cause = result.cause.as_deref().map(|cause| {
        if CAUSES.contains(&cause) || (model && Cause::is_code(cause)) {
            cause.to_owned()
        } else {
            UNKNOWN_CAUSE.to_owned()
        }
    });
    Some(WorkResult {
        cause,
        ..result.clone()
    })
}

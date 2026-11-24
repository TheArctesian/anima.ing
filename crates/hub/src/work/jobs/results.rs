//! The result of an attempt as the hub keeps it (K10 Section 7): fixed codes and numbers, never text from the job or the machine.
//! A machine, or a person with its daemon token, could otherwise put text of its choice where a screen or an MCP tool shows it.

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::files::MAX_FILES;
use daemon_core::model_work::found::{MAX_FOUND, Tool, is_shown_name};
use daemon_core::model_work::memory::keep::IDLE_SECONDS;
use daemon_core::model_work::names::{Word as _, is_lower_hex};
use daemon_core::wire::{
    Cleanup, CopyOutcome, FoundEntry, LoadEntry, ModelOutcome, ModelState, ServerLoads, WorkKind,
    WorkResult,
};

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
    let outcome = match &result.model {
        None => None,
        Some(_) if !model => return None,
        Some(ModelOutcome::Copy(copy)) => Some(ModelOutcome::Copy(checked_copy(copy)?)),
        Some(ModelOutcome::State(state)) => Some(ModelOutcome::State(checked_state(state)?)),
    };
    if !cleaned || !OUTCOMES.contains(&result.outcome.as_str()) {
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
        model: outcome,
        ..result.clone()
    })
}

/// A copy outcome of a state of K11 Section 11, with at most one hash for each file.
fn checked_copy(copy: &CopyOutcome) -> Option<CopyOutcome> {
    let hashes_fit = copy.file_hashes.len() <= MAX_FILES
        && copy
            .file_hashes
            .iter()
            .all(|hash| is_lower_hex(&hash.sha256, 64));
    (COPY_STATES.contains(&copy.copy_state.as_str()) && hashes_fit).then(|| copy.clone())
}

/// The states of a copy in the memory of a server (K11 Section 11.1).
const LOAD_STATES: [&str; 4] = ["loading", "ready", "active", "unloading"];

/// The most servers of a node, and the most copies of a server, that the hub keeps (K11 Section 11.1).
const MAX_SERVERS: usize = 16;
const MAX_LOADS: usize = 16;

/// The found models and the loads of a state that keep the rules of K11 Sections 8 and 11.1: the
/// others are left out, so a machine cannot put text of its choice where the screen shows them.
/// `None` when the time is not RFC 3339.
fn checked_state(state: &ModelState) -> Option<ModelState> {
    chrono::DateTime::parse_from_rfc3339(&state.at).ok()?;
    let found = state
        .found
        .iter()
        .filter(|entry| found_fits(entry))
        .take(MAX_FOUND)
        .cloned()
        .collect();
    let servers = state
        .servers
        .iter()
        .take(MAX_SERVERS)
        .map(|server| ServerLoads {
            server: server.server,
            models: server
                .models
                .iter()
                .filter(|entry| load_fits(entry))
                .take(MAX_LOADS)
                .cloned()
                .collect(),
        })
        .collect();
    Some(ModelState {
        at: state.at.clone(),
        found,
        servers,
    })
}

fn load_fits(entry: &LoadEntry) -> bool {
    LOAD_STATES.contains(&entry.state.as_str()) && IDLE_SECONDS.contains(&entry.idle_seconds)
}

fn found_fits(entry: &FoundEntry) -> bool {
    let format_fits = match entry.format.as_str() {
        "gguf" => true,
        "safetensors" => !entry.usable,
        _ => false,
    };
    is_lower_hex(&entry.found, 64)
        && Tool::parse(&entry.tool).is_some()
        && is_shown_name(&entry.name)
        && format_fits
        && (1..=u32::try_from(MAX_FILES).unwrap_or(u32::MAX)).contains(&entry.files)
}

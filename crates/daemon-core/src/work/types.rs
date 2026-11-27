//! The object and secrets of each work type (K10 Section 6); the list is fixed here and the hub cannot add a type.

use crate::wire::WorkKind;
use serde_json::{Map, Value};

use super::shape::{has_exactly, text};

type Object = Map<String, Value>;

/// The version of a runner program, as GitHub numbers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('.').map(|part| {
            let digits = !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
            digits.then(|| part.parse::<u64>().ok()).flatten()
        });
        let version = Self(parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(version)
    }
}

fn is_hex(text: &str, length: usize) -> bool {
    text.len() == length && text.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_word(text: &str) -> bool {
    (1..=64).contains(&text.len())
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+'))
}

/// Checks the closed build parameters and origin constraint (SF-009).
/// Callers separately enforce permitted secret purposes and machine admission.
#[must_use]
pub fn build_holds(parameters: &Object, secrets: &[&str], kind: WorkKind) -> bool {
    let origin = text(parameters, "origin");
    recipe_holds(parameters, kind)
        && text(parameters, "commit").is_some_and(|commit| is_hex(commit, 40))
        && text(parameters, "trigger")
            .is_some_and(|trigger| ["operator", "push"].contains(&trigger))
        && origin.is_some_and(|origin| ["default_branch", "other"].contains(&origin))
        && !(origin == Some("other") && secrets.contains(&"project_secret"))
}

fn recipe_holds(parameters: &Object, kind: WorkKind) -> bool {
    let (member, valid) = match text(parameters, "recipe") {
        Some("desktop_release") => (
            "version",
            text(parameters, "target").is_some_and(is_word)
                && text(parameters, "version").is_some_and(is_word),
        ),
        Some("source_image_v1") => (
            "strategy",
            kind == WorkKind::Container
                && text(parameters, "target")
                    .is_some_and(|target| ["linux-x86_64", "linux-aarch64"].contains(&target))
                && text(parameters, "strategy")
                    .is_some_and(|strategy| ["auto", "dockerfile", "railpack"].contains(&strategy)),
        ),
        _ => return false,
    };
    valid
        && has_exactly(
            parameters,
            &["recipe", "commit", "target", member, "trigger", "origin"],
        )
}

pub(super) fn runner_version(parameters: &Object) -> Option<Version> {
    const MEMBERS: [&str; 3] = ["runnerVersion", "runnerSha256", "labels"];
    let labels = parameters.get("labels")?.as_array()?;
    let labels_hold = labels.len() <= 16
        && labels
            .iter()
            .all(|label| label.as_str().is_some_and(is_word));
    let holds = has_exactly(parameters, &MEMBERS)
        && text(parameters, "runnerSha256").is_some_and(|sha| is_hex(sha, 64))
        && labels_hold;
    holds
        .then(|| text(parameters, "runnerVersion").and_then(Version::parse))
        .flatten()
}

/// The purposes that an item of the type can name (K10 Sections 6 and 8).
pub(super) fn purposes_of(work_type: &str) -> &'static [&'static str] {
    match work_type {
        "build" => &["repository_token", "project_secret"],
        "runner" => &["jit_config"],
        _ => &[],
    }
}

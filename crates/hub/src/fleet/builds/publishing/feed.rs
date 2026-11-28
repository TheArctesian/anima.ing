//! The feed check and the signed feed (BLD-065, BLD-067 to BLD-069, UPD-009, R-117). Pure.
//! A refusal names no builder text but checked file names and versions.

use serde_json::{Map, Value};

use crate::fleet::builds::{
    model::{Target, UploadedFile},
    version::Version,
};

/// The file the job lists its digests in (UPD-010).
pub(super) const SUMS: &str = "SHA256SUMS";

const MAX_NOTES_BYTES: usize = 1024;
pub(super) const MAX_FEED_BYTES: usize = 64 * 1024;

pub(super) fn published_version(bytes: &[u8]) -> Option<Version> {
    let top: Value = serde_json::from_slice(bytes).ok()?;
    top.get("version")?.as_str()?.parse().ok()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CheckedFeed {
    pub(super) updater: String,
}

/// Also the folders whose `latest.json` the hub writes. A universal macOS bundle serves both
/// Mac architectures (BLD-067, BLD-068).
pub(super) const fn platform_keys(target: Target) -> &'static [&'static str] {
    match target {
        Target::LinuxX86_64 => &["linux-x86_64"],
        Target::WindowsX86_64 => &["windows-x86_64"],
        Target::DarwinUniversal => &["darwin-aarch64", "darwin-x86_64"],
    }
}

const fn updater_suffix(target: Target) -> &'static str {
    match target {
        Target::LinuxX86_64 => ".AppImage",
        Target::WindowsX86_64 => "-setup.exe",
        Target::DarwinUniversal => ".app.tar.gz",
    }
}

pub(super) fn updater_name(target: Target, files: &[UploadedFile]) -> Option<&str> {
    let mut updaters = files
        .iter()
        .filter(|file| file.name.ends_with(updater_suffix(target)));
    match (updaters.next(), updaters.next()) {
        (Some(file), None) => Some(&file.name),
        _ => None,
    }
}

pub(super) fn signature_name(updater: &str) -> String {
    format!("{updater}.sig")
}

pub(super) struct Expected<'a> {
    pub(super) target: Target,
    pub(super) version: Version,
    /// No `/` at the end.
    pub(super) public_base: &'a str,
    pub(super) files: &'a [UploadedFile],
    /// The highest version `target` has published (D-27).
    pub(super) floor: Option<Version>,
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key).and_then(Value::as_str)
}

fn check_top(top: &Map<String, Value>, expected: &Expected<'_>) -> Result<(), String> {
    let known = ["version", "notes", "pub_date", "platforms"];
    if top.keys().any(|key| !known.contains(&key.as_str())) {
        return Err(
            "The feed has a field other than version, notes, pub_date and platforms.".into(),
        );
    }
    if text(top, "version") != Some(expected.version.to_string().as_str()) {
        return Err(format!("The feed is not for version {}.", expected.version));
    }
    let notes_ok = top
        .get("notes")
        .is_none_or(|notes| notes.as_str().is_some_and(|n| n.len() <= MAX_NOTES_BYTES));
    let date_ok = top.get("pub_date").is_none_or(|date| {
        date.as_str()
            .is_some_and(|d| chrono::DateTime::parse_from_rfc3339(d).is_ok())
    });
    if !notes_ok || !date_ok {
        return Err("The feed's notes or pub_date are not valid.".into());
    }
    if let Some(floor) = expected.floor.filter(|floor| expected.version < *floor) {
        return Err(format!(
            "{} has published {floor}, so a feed of {} would go back.",
            expected.target, expected.version
        ));
    }
    Ok(())
}

/// A signature is refused: the hub signs the file it checked (R-117).
fn check_entry(entry: &Value, expected: &Expected<'_>) -> Result<CheckedFeed, String> {
    let refused = || {
        format!(
            "Each feed entry needs only a URL under {}/{}/{}/; the hub signs the updater file.",
            expected.public_base, expected.target, expected.version
        )
    };
    let entry = entry
        .as_object()
        .filter(|e| e.len() == 1)
        .ok_or_else(refused)?;
    let folder = format!(
        "{}/{}/{}/",
        expected.public_base, expected.target, expected.version
    );
    let name = text(entry, "url")
        .and_then(|url| url.strip_prefix(&folder))
        .ok_or_else(refused)?;
    if !expected.files.iter().any(|file| file.name == name) {
        return Err(refused());
    }
    if !name.ends_with(updater_suffix(expected.target)) {
        return Err(format!(
            "The feed does not name the updater file of {}.",
            expected.target
        ));
    }
    Ok(CheckedFeed {
        updater: name.to_owned(),
    })
}

pub(super) fn check(feed: &str, expected: &Expected<'_>) -> Result<CheckedFeed, String> {
    let Ok(Value::Object(top)) = serde_json::from_str::<Value>(feed) else {
        return Err("The feed is not a JSON object.".into());
    };
    check_top(&top, expected)?;
    let keys = platform_keys(expected.target);
    let platforms = top.get("platforms").and_then(Value::as_object);
    let listed: Vec<&str> =
        platforms.map_or_else(Vec::new, |p| p.keys().map(String::as_str).collect());
    if listed.len() != keys.len() || !keys.iter().all(|key| listed.contains(key)) {
        return Err(format!(
            "The feed of {} must list exactly {}.",
            expected.target,
            keys.join(" and ")
        ));
    }
    let mut checked = None;
    for entry in platforms.into_iter().flat_map(Map::values) {
        let this = check_entry(entry, expected)?;
        if checked.as_ref().is_some_and(|first| *first != this) {
            return Err("The feed's entries must name the same file.".into());
        }
        checked = Some(this);
    }
    checked.ok_or_else(|| "The feed lists no platform.".into())
}

/// The same feed and signature always give the same text, so a later try can confirm a feed
/// an earlier one wrote (D-25).
pub(super) fn signed(feed: &str, signature: &str) -> Result<String, String> {
    let refused = || "The feed is not a JSON object with platforms.".to_owned();
    let mut top: Value = serde_json::from_str(feed).map_err(|_| refused())?;
    let platforms = top
        .get_mut("platforms")
        .and_then(Value::as_object_mut)
        .ok_or_else(refused)?;
    for entry in platforms.values_mut() {
        let entry = entry.as_object_mut().ok_or_else(refused)?;
        entry.insert("signature".to_owned(), Value::String(signature.to_owned()));
    }
    Ok(top.to_string())
}

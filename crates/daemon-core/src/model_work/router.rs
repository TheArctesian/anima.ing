//! A `llama_server` in router mode (RTC-005; K11 Section 11.1): the preset that names the copies of a server,
//! and the answers of `GET /models` and `GET /metrics?model=<copy>`. Pure: the daemon does the I/O.
//!
//! The router reads its preset at its start and again at `GET /models?reload=1`. A reload keeps a loaded copy
//! whose section did not change, unloads a copy whose section changed, and removes a copy whose section is gone;
//! so the daemon writes the sections of one server in one fixed order and changes a section only by a load.

#[cfg(test)]
mod tests;

use std::fmt::Write as _;
use std::path::Path;

use serde_json::Value;
use uuid::Uuid;

use super::command::llama_settings;
use super::settings::ServerSettings;

/// One copy of a server's preset: its model file, its projector if it has one, and the settings of its load.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry<'a> {
    pub copy: Uuid,
    pub model: &'a Path,
    pub projector: Option<&'a Path>,
    pub settings: &'a ServerSettings,
}

/// A path that the INI text holds as it is: absolute, text, and with no character that ends a line or a value.
fn plain(path: &Path) -> Option<&str> {
    let text = path.to_str()?;
    let holds = path.is_absolute()
        && !text
            .chars()
            .any(|c| c.is_control() || matches!(c, '[' | ']' | ';' | '#'));
    holds.then_some(text)
}

/// The preset text of `entries`, sorted by copy; `None` when a path does not hold as plain text.
#[must_use]
pub fn preset(entries: &[Entry<'_>]) -> Option<String> {
    let mut sorted: Vec<&Entry<'_>> = entries.iter().collect();
    sorted.sort_by_key(|entry| entry.copy);
    let mut text = String::from("version = 1\n");
    for entry in sorted {
        let _ = write!(
            text,
            "\n[{}]\nmodel = {}\n",
            entry.copy,
            plain(entry.model)?
        );
        if let Some(projector) = entry.projector {
            let _ = writeln!(text, "mmproj = {}", plain(projector)?);
        }
        for (name, value) in llama_settings(entry.settings) {
            let _ = writeln!(text, "{name} = {value}");
        }
    }
    Some(text)
}

fn is_projector(name: &str) -> bool {
    name.to_ascii_lowercase().starts_with("mmproj")
}

/// The model file and the projector of a GGUF copy from the names of its files: the one `.gguf` that is not a
/// projector, or the first part of a split model (`-00001-of-`). `None` when that is not one file.
#[must_use]
pub fn model_files(names: &[String]) -> Option<(&str, Option<&str>)> {
    let gguf: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|name| name.to_ascii_lowercase().ends_with(".gguf"))
        .collect();
    let (projectors, models): (Vec<&str>, Vec<&str>) =
        gguf.into_iter().partition(|name| is_projector(name));
    let model = match models.as_slice() {
        [one] => *one,
        many => {
            let first: Vec<&str> = many
                .iter()
                .copied()
                .filter(|name| name.contains("-00001-of-"))
                .collect();
            match first.as_slice() {
                [one] => *one,
                _ => return None,
            }
        }
    };
    match projectors.as_slice() {
        [] => Some((model, None)),
        [one] => Some((model, Some(*one))),
        _ => None,
    }
}

/// The state of a copy in the router.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Loading,
    Loaded,
    Unloaded,
    /// The last load ended with an error of the runtime.
    Failed,
}

/// The copies of a `GET /models` answer: only the sections of the preset, by copy.
#[must_use]
pub fn statuses(answer: &Value) -> Vec<(Uuid, Status)> {
    let Some(list) = answer.get("data").and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter()
        .filter(|model| model.get("source").and_then(Value::as_str) == Some("preset"))
        .filter_map(|model| {
            let copy = Uuid::parse_str(model.get("id")?.as_str()?).ok()?;
            let status = model.get("status")?;
            let failed = status.get("failed").and_then(Value::as_bool) == Some(true);
            let state = match status.get("value")?.as_str()? {
                "loading" => Status::Loading,
                "loaded" | "sleeping" => Status::Loaded,
                _ if failed => Status::Failed,
                _ => Status::Unloaded,
            };
            Some((copy, state))
        })
        .collect()
}

/// The use of a copy from its `/metrics`: the requests in progress or waiting, and a counter that grows with
/// each request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Use {
    pub in_progress: u64,
    pub decodes: u64,
}

/// `None` when the text has no counter of a request.
#[must_use]
pub fn usage(metrics: &str) -> Option<Use> {
    let value = |name: &str| {
        metrics.lines().find_map(|line| {
            let (key, number) = line.split_once(' ')?;
            (key == name).then(|| number.trim().parse::<u64>().ok())?
        })
    };
    let processing = value("llamacpp:requests_processing")?;
    Some(Use {
        in_progress: processing.saturating_add(value("llamacpp:requests_deferred").unwrap_or(0)),
        decodes: value("llamacpp:n_decode_total")?,
    })
}

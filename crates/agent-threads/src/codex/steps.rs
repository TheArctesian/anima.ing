//! The `item_completed` events of the newer Codex rollouts: `UserMessage` and `AgentMessage`
//! hold the prompts and the replies as the person saw them; `CommandExecution` and
//! `FileChange` hold the commands and the changes of files, with their outputs. A
//! `Reasoning` step repeats the `reasoning` response item, so it is not read.

use serde_json::Value;

use super::super::{ThreadReader, Time, tool};
use super::scripts;

/// The most files of one change that are shown; the others are counted.
const MAX_FILES: usize = 64;

pub(super) fn completed(reader: &mut ThreadReader, at: Option<Time>, step: &Value) {
    let field = |name: &str| step.get(name).and_then(Value::as_str).unwrap_or_default();
    match field("type") {
        "UserMessage" => {
            let parts = parts(step);
            let images = parts
                .iter()
                .filter(|part| matches!(kind(part), "image" | "local_image"))
                .count();
            scripts::close_all(reader);
            reader.prompt(
                at,
                &texts(&parts, "text"),
                u32::try_from(images).unwrap_or(u32::MAX),
            );
        }
        "AgentMessage" => reader.text(at, false, &texts(&parts(step), "Text")),
        "CommandExecution" => {
            let command = command(step.get("command"));
            let failed = field("status") != "completed"
                || step
                    .get("exit_code")
                    .and_then(Value::as_i64)
                    .is_some_and(|code| code != 0);
            let input = serde_json::json!({ "command": command, "cwd": field("cwd") });
            let mut call = tool::call(field("id"), "exec_command", &input, &command);
            (call.output, call.failed) = (Some(field("aggregated_output").to_owned()), failed);
            scripts::step(reader, at, call);
        }
        "FileChange" => {
            let patch = patch(step.get("changes"));
            let output = [field("stdout"), field("stderr")]
                .into_iter()
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            let mut call = tool::call(field("id"), "apply_patch", &Value::Null, &patch);
            (call.output, call.failed) = (Some(output), field("status") != "completed");
            scripts::step(reader, at, call);
        }
        _ => {}
    }
}

fn parts(step: &Value) -> Vec<&Value> {
    step.get("content")
        .and_then(Value::as_array)
        .map_or_default(|parts| parts.iter().collect())
}

fn kind(part: &Value) -> &str {
    part.get("type").and_then(Value::as_str).unwrap_or_default()
}

/// The text of the parts of the kind `text_kind`, one after the other.
fn texts(parts: &[&Value], text_kind: &str) -> String {
    parts
        .iter()
        .filter(|part| kind(part) == text_kind)
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The command as the person would type it: the script of `<shell> -lc <script>`, else the
/// words joined.
fn command(words: Option<&Value>) -> String {
    let words = words
        .and_then(Value::as_array)
        .map_or_default(|words| words.iter().filter_map(Value::as_str).collect::<Vec<_>>());
    match words.as_slice() {
        [_, flag, script] if matches!(*flag, "-lc" | "-c") => (*script).to_owned(),
        _ => words.join(" "),
    }
}

/// The changes of files as one patch in the form of Codex: a head line for each file, then its
/// lines, added with `+`, removed with `-`, or the diff of an update as Codex wrote it.
fn patch(changes: Option<&Value>) -> String {
    let Some(changes) = changes.and_then(Value::as_object) else {
        return String::new();
    };
    let mut out = Vec::new();
    for (path, change) in changes.iter().take(MAX_FILES) {
        let text = |name: &str| change.get(name).and_then(Value::as_str).unwrap_or_default();
        let marked = |mark: char| {
            text("content")
                .lines()
                .map(|line| format!("{mark}{line}"))
                .collect::<Vec<_>>()
        };
        match text("type") {
            "add" => {
                out.push(format!("*** Add File: {path}"));
                out.extend(marked('+'));
            }
            "delete" => {
                out.push(format!("*** Delete File: {path}"));
                out.extend(marked('-'));
            }
            _ => {
                out.push(format!("*** Update File: {path}"));
                out.extend(text("unified_diff").lines().map(str::to_owned));
            }
        }
    }
    if changes.len() > MAX_FILES {
        out.push(format!("[{} more files]", changes.len() - MAX_FILES));
    }
    out.join("\n")
}

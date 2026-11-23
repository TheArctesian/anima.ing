//! The `item_completed` events of the newer Codex rollouts: `UserMessage` and `AgentMessage`
//! hold the prompts and the replies as the person saw them; `CommandExecution` and
//! `FileChange` hold the commands and the changes of files, with their outputs. A
//! `Reasoning` item repeats the `reasoning` response item, so it is not read.

use serde_json::Value;

use super::super::{MAX_OUTPUT, ThreadReader, Time, ToolCall, tool};

/// The most files of one change that are shown; the others are counted.
const MAX_FILES: usize = 64;

pub(super) fn completed(reader: &mut ThreadReader, at: Option<Time>, item: &Value) {
    let field = |name: &str| item.get(name).and_then(Value::as_str).unwrap_or_default();
    match field("type") {
        "UserMessage" => {
            let parts = parts(item);
            let images = parts
                .iter()
                .filter(|part| matches!(kind(part), "image" | "local_image"))
                .count();
            reader.prompt(
                at,
                &texts(&parts, "text"),
                u32::try_from(images).unwrap_or(u32::MAX),
            );
        }
        "AgentMessage" => reader.text(at, false, &texts(&parts(item), "Text")),
        "CommandExecution" => {
            let command = command(item.get("command"));
            let failed = field("status") != "completed"
                || item
                    .get("exit_code")
                    .and_then(Value::as_i64)
                    .is_some_and(|code| code != 0);
            let input = serde_json::json!({ "command": command, "cwd": field("cwd") });
            let mut call = tool::call(field("id"), "exec_command", &input, &command);
            (call.output, call.failed) = (Some(field("aggregated_output").to_owned()), failed);
            done(reader, at, call);
        }
        "FileChange" => {
            let patch = patch(item.get("changes"));
            let output = [field("stdout"), field("stderr")]
                .into_iter()
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            let mut call = tool::call(field("id"), "apply_patch", &Value::Null, &patch);
            (call.output, call.failed) = (Some(output), field("status") != "completed");
            done(reader, at, call);
        }
        _ => {}
    }
}

/// A call that came with its result. Inside a script of `exec` it is a step of that call: its
/// output and its failure go to the call of the script, which is already shown.
fn done(reader: &mut ThreadReader, at: Option<Time>, mut call: ToolCall) {
    let (output, failed) = (call.output.take().unwrap_or_default(), call.failed);
    if let Some(script) = reader.script.as_mut() {
        script.failed |= failed;
        // The result is held to its limit; what lies past it is not gathered.
        if script.output.len() > MAX_OUTPUT {
            return;
        }
        if !script.output.is_empty() {
            script.output.push('\n');
        }
        script.output.push_str(&output);
        return;
    }
    let id = call.call.clone();
    reader.call(at, call);
    reader.result(&id, &output, failed);
}

fn parts(item: &Value) -> Vec<&Value> {
    item.get("content")
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

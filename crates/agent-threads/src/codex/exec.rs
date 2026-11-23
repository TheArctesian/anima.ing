//! The `exec` tool of the newer Codex rollouts, read as text: the agent sends a short script
//! that calls the tools of Codex (`tools.exec_command({cmd: "..."})`,
//! `tools.apply_patch("...")`, `tools.write_stdin({session_id: 1, chars: ""})`), and the output
//! of the script says how it ended. Nothing in a script is run or opened.

use serde_json::{Deserializer, Value, json};

/// The tools of a script that this reader names a call for.
const TOOLS: [&str; 3] = ["exec_command", "apply_patch", "write_stdin"];

/// The tool the script calls first, with the input this reader takes from it; `None` when the
/// script calls no tool this reader knows, or its argument is not in a form it reads.
pub(super) fn first_tool(script: &str) -> Option<(&'static str, Value)> {
    let (start, name) = TOOLS
        .iter()
        .filter_map(|name| Some((script.find(&format!("tools.{name}("))?, *name)))
        .min()?;
    let argument = &script[start + "tools.(".len() + name.len()..];
    let input = match name {
        "apply_patch" => json!({ "input": literal(argument)? }),
        "exec_command" => json!({ "cmd": literal(field(argument, "cmd")?)? }),
        _ => {
            let session = field(argument, "session_id")?;
            let digits = session
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(session.len());
            let chars = field(argument, "chars")
                .and_then(literal)
                .unwrap_or_default();
            json!({ "session_id": session[..digits].parse::<u64>().ok()?, "chars": chars })
        }
    };
    Some((name, input))
}

/// The text after `key:` (or `"key":`) in an object of a script, where the key starts a member,
/// not a word inside a string.
fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(found) = text[from..].find(key) {
        let at = from + found;
        from = at + key.len();
        let before = text[..at].trim_end_matches('"').chars().next_back();
        if !matches!(before, Some('{' | ',' | ' ' | '\n')) {
            continue;
        }
        if let Some(value) = text[from..]
            .trim_start_matches('"')
            .trim_start()
            .strip_prefix(':')
        {
            return Some(value);
        }
    }
    None
}

/// The JSON string at the start of `text` (after blanks): a script writes its strings in the
/// form JSON reads, and any other form is not read.
fn literal(text: &str) -> Option<String> {
    let text = text.trim_start();
    if !text.starts_with('"') {
        return None;
    }
    Deserializer::from_str(text)
        .into_iter::<String>()
        .next()?
        .ok()
}

/// How the output of a script ended.
pub(super) struct Ended {
    /// What the output shows: the outputs of its commands, else its text.
    pub shown: String,
    pub failed: bool,
    /// The cell of a script that still runs (`Script running with cell ID <cell>`): a `wait`
    /// names it and carries the rest of its output.
    pub cell: Option<String>,
    /// The session of a command that still runs: a `write_stdin` names it.
    pub session: Option<u64>,
}

/// Reads the output of a script: its first line says whether it completed, failed, was stopped
/// or still runs; after `Output:` come the JSON results of its commands, one per part, each
/// with its `output` and `exit_code`.
pub(super) fn ended(text: &str) -> Ended {
    let head = text.lines().next().unwrap_or_default();
    let cell = head
        .strip_prefix("Script running with cell ID ")
        .map(|cell| crate::first_line(cell, crate::NAME_LENGTH));
    let mut failed = head.starts_with("Script failed")
        || head.starts_with("aborted")
        || head.contains(" failed");
    let body = text.split_once("\nOutput:\n").map_or("", |(_, body)| body);
    let results: Option<Vec<Value>> = Deserializer::from_str(body)
        .into_iter::<Value>()
        .map(|value| value.ok().filter(Value::is_object))
        .collect();
    let results = results.unwrap_or_default();
    let mut shown = Vec::new();
    let mut session = None;
    for result in &results {
        failed |= result
            .get("exit_code")
            .and_then(Value::as_i64)
            .is_some_and(|code| code != 0);
        session = session.or_else(|| result.get("session_id").and_then(Value::as_u64));
        if let Some(output) = result.get("output").and_then(Value::as_str) {
            shown.push(output);
        }
    }
    // A script whose results hold no output (`{}`) shows its first line.
    let shown = match (shown.is_empty(), results.is_empty()) {
        (false, _) => shown.concat(),
        (true, false) => head.to_owned(),
        (true, true) => text.to_owned(),
    };
    Ended {
        shown,
        failed,
        cell,
        session,
    }
}

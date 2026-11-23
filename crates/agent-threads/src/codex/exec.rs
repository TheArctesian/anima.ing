//! The `exec` tool of the newer Codex rollouts: the agent sends a short script that calls the
//! tools of Codex (`tools.exec_command({cmd: "..."})`, `tools.apply_patch("...")`), and the
//! output says how the script ended. The script is only read as text: the first tool it calls
//! names the call, and nothing in it is run or opened.

use serde_json::{Deserializer, Value, json};

/// The tool the script calls first, with the input this reader takes from it; `None` when the
/// script calls no tool this reader knows, or its argument is not a plain string.
pub(super) fn first_tool(script: &str) -> Option<(&'static str, Value)> {
    let (name, rest) = ["exec_command", "apply_patch"].iter().find_map(|name| {
        let start = script.find(&format!("tools.{name}("))?;
        Some((*name, &script[start + name.len() + 7..]))
    })?;
    if name == "apply_patch" {
        return Some((name, json!({ "input": literal(rest)? })));
    }
    let key = rest.find("cmd")?;
    let after = rest[key + 3..].trim_start_matches(['"', '\'']).trim_start();
    let value = after.strip_prefix(':')?;
    Some((name, json!({ "cmd": literal(value)? })))
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

/// What the output of a script shows and whether it failed: the text after `Output:`, or the
/// `output` of the JSON result there, with its `exit_code`. A script that failed, was stopped,
/// or ended in an error of the runtime says so in its first line.
pub(super) fn ended(text: &str) -> (String, bool) {
    let head = text.lines().next().unwrap_or_default();
    let mut failed = head.starts_with("Script failed")
        || head.starts_with("aborted")
        || head.contains(" failed");
    let body = text
        .split_once("\nOutput:\n")
        .map_or(text, |(_, body)| body)
        .trim();
    let result = serde_json::from_str::<Value>(body)
        .ok()
        .filter(Value::is_object);
    if let Some(result) = result {
        failed |= result
            .get("exit_code")
            .and_then(Value::as_i64)
            .is_some_and(|code| code != 0);
        if let Some(output) = result.get("output").and_then(Value::as_str) {
            return (output.to_owned(), failed);
        }
        if result.as_object().is_some_and(serde_json::Map::is_empty) {
            return (head.to_owned(), failed);
        }
    }
    (text.to_owned(), failed)
}

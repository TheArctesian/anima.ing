//! The scripts of `exec` that are open in a Codex rollout, so that what belongs to a script
//! goes to its one call: the commands and changes of files that Codex reports while it runs,
//! the `wait` that carries the rest of the output of a script that still ran, and the
//! `write_stdin` poll that reads more of a command that still ran.

use serde_json::Value;

use super::super::{MAX_OUTPUT, ThreadReader, Time, ToolCall, held, tool};
use super::exec::{self, Ended};

/// The most scripts open at once, and the most commands that still run, that are kept: a
/// file that opens more drops the oldest.
const MAX_OPEN: usize = 16;

/// The open scripts of one rollout, the newest last.
#[derive(Debug, Default)]
pub(in super::super) struct Scripts {
    open: Vec<Script>,
    /// The call of each command that still runs, by its session.
    sessions: Vec<(u64, String)>,
}

#[derive(Debug)]
struct Script {
    /// The call that shows the script.
    call: String,
    /// The `wait` or the `write_stdin` poll whose output ends the script, by its call ID.
    waiter: Option<String>,
    /// The cell of the script while it runs past its first output.
    cell: Option<String>,
    /// The output of the commands of the script, as Codex reported them.
    output: String,
    failed: bool,
    /// A poll: its output is more of a call that already has a result.
    more: bool,
}

/// A new script with the call `id`: a call named for its first tool, or, for a poll of a
/// command that still runs, more of the call of that command.
pub(super) fn begin(reader: &mut ThreadReader, at: Option<Time>, id: &str, raw: &str) {
    let first = exec::first_tool(raw);
    // A poll writes nothing and calls no other tool: anything else is a call of its own.
    let poll = first.as_ref().filter(|(name, input)| {
        *name == "write_stdin" && input["chars"] == "" && raw.matches("tools.").count() == 1
    });
    if let Some((_, input)) = poll {
        let session = input.get("session_id").and_then(Value::as_u64);
        let polled = reader
            .scripts
            .sessions
            .iter()
            .rev()
            .find(|(id, _)| Some(*id) == session);
        if let Some((_, call)) = polled {
            let call = call.clone();
            // The command runs again until the poll ends.
            reader.open.insert(call.clone());
            reader.thread.tail = crate::Tail::OpenTool;
            open(reader, Script::new(&call, Some(id), true));
            return;
        }
    }
    let mut call = match first {
        Some((name, input)) => {
            let text = input.get("input").and_then(Value::as_str).unwrap_or(raw);
            tool::call(id, name, &input, text)
        }
        None => tool::call(id, "exec", &Value::Null, raw),
    };
    call.input = held(raw, super::super::MAX_TEXT);
    open(reader, Script::new(id, None, false));
    reader.call(at, call);
}

/// A `wait` with the call `id` for the script of `cell`; false when no open script has it.
pub(super) fn wait(reader: &mut ThreadReader, id: &str, cell: &str) -> bool {
    let Some(script) = reader
        .scripts
        .open
        .iter_mut()
        .find(|script| script.cell.as_deref() == Some(cell))
    else {
        return false;
    };
    script.waiter = Some(id.to_owned());
    true
}

/// The output `text` of the call `id` when it is the call or the waiter of an open script;
/// false when it is not.
pub(super) fn output(reader: &mut ThreadReader, id: &str, text: &str) -> bool {
    let Some(index) = reader.scripts.open.iter().position(|script| {
        script
            .waiter
            .as_deref()
            .map_or(script.call == id, |waiter| waiter == id)
    }) else {
        return false;
    };
    let ended = exec::ended(text);
    if let Some(cell) = ended.cell {
        let script = &mut reader.scripts.open[index];
        (script.cell, script.waiter) = (Some(cell), None);
        script.failed |= ended.failed;
        return true;
    }
    let script = reader.scripts.open.remove(index);
    let Ended {
        shown,
        failed,
        session,
        reported,
        ..
    } = ended;
    // A command that still runs can be polled; one that ended cannot.
    let sessions = &mut reader.scripts.sessions;
    sessions.retain(|(_, call)| *call != script.call);
    if let Some(session) = session {
        sessions.push((session, script.call.clone()));
        if sessions.len() > MAX_OPEN {
            sessions.remove(0);
        }
    }
    let failed = failed || script.failed;
    // The results the script reported are its own; the steps Codex reported fill in for a
    // script whose results hold no output (a change of a file answers `{}`).
    let shown = if reported || script.output.is_empty() {
        shown
    } else {
        script.output
    };
    if script.more {
        reader.more(&script.call, &shown, failed);
    } else {
        reader.result(&script.call, &shown, failed);
    }
    true
}

/// A command or a change of a file that Codex reports: a step of the newest open script, or a
/// call of its own when no script is open.
pub(super) fn step(reader: &mut ThreadReader, at: Option<Time>, mut call: ToolCall) {
    let (output, failed) = (call.output.take().unwrap_or_default(), call.failed);
    if let Some(script) = reader.scripts.open.last_mut() {
        script.failed |= failed;
        let room = MAX_OUTPUT.saturating_sub(script.output.len());
        if room > 0 {
            if !script.output.is_empty() {
                script.output.push('\n');
            }
            script.output.push_str(&held(&output, room));
        }
        return;
    }
    let id = call.call.clone();
    reader.call(at, call);
    reader.result(&id, &output, failed);
}

/// No script stays open past a prompt, a compaction or the end of a turn.
pub(super) fn close_all(reader: &mut ThreadReader) {
    reader.scripts.open.clear();
}

fn open(reader: &mut ThreadReader, script: Script) {
    let open = &mut reader.scripts.open;
    open.push(script);
    if open.len() > MAX_OPEN {
        open.remove(0);
    }
}

impl Script {
    fn new(call: &str, waiter: Option<&str>, more: bool) -> Self {
        Self {
            call: call.to_owned(),
            waiter: waiter.map(str::to_owned),
            cell: None,
            output: String::new(),
            failed: false,
            more,
        }
    }
}

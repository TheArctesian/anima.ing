//! A program that the runner starts by itself to ask it one thing (TH104, TH117): its models, or
//! the skills of a folder. It starts in `/`, which it cannot write and which has no project
//! files; the runner ends its process group after the answer or after [`ASK_WAIT`]. Claude Code
//! reads no settings of the user (no hook) and starts no MCP server.

use std::path::PathBuf;
use std::time::Duration;

use agent_threads::drive::Program;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _};

/// The longest wait for an answer. Codex loads its plugins first (about 90 s on a busy Mac).
const ASK_WAIT: Duration = Duration::from_secs(120);
/// The most bytes of one line of the program that the ask reads.
const MOST_LINE: usize = 1 << 20;
/// The most lines that the ask reads before it gives up.
const MOST_LINES: usize = 200;

/// Why an ask gave no answer, in fixed words that hold no path, token or text of the program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    NotStarted,
    Ended,
    TooMuch,
    Slow,
}

impl Fault {
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Self::NotStarted => "The program did not start.",
            Self::Ended => "The program ended before it answered.",
            Self::TooMuch => "The program wrote too much before it answered.",
            Self::Slow => "The program did not answer within two minutes.",
        }
    }
}

/// The program to ask: its path, found as a thread finds it, and the environment of a thread.
pub struct Asked {
    pub program: Program,
    pub path: PathBuf,
    pub env: Vec<(String, String)>,
}

/// What one line of the program asks of the ask.
pub enum Heard<T> {
    /// The answer: the ask ends.
    Done(T),
    /// Lines to write to the program, then read on.
    Write(Vec<String>),
    Nothing,
}

/// Whether `value` is the answer of Codex to `initialize` (ID 1): the ask goes on after it.
pub fn initialized(value: &Value) -> bool {
    value["id"] == 1 && value.get("method").is_none()
}

/// Starts `program`, writes `first`, and reads its lines into `heard` until it gives the answer.
///
/// # Errors
/// The [`Fault`] when the program does not start, ends, says too much, or is too slow.
pub async fn ask<T>(
    asked: Asked,
    first: String,
    heard: impl FnMut(&Value) -> Heard<T>,
) -> Result<T, Fault> {
    let Asked { program, path, env } = asked;
    let args: &[&str] = match program {
        Program::ClaudeCode => &[
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            "--permission-mode",
            "plan",
            "--strict-mcp-config",
            "--setting-sources",
            "project,local",
            "--no-session-persistence",
        ],
        Program::Codex => &["app-server"],
        Program::Grok => &["agent", "--no-leader", "stdio"],
    };
    let mut child = tokio::process::Command::new(&path)
        .args(args)
        .current_dir("/")
        .env_clear()
        .envs(env)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| Fault::NotStarted)?;
    crate::threads::priority::lift_async(&mut child).await;
    let group = child
        .id()
        .and_then(|pid| i32::try_from(pid).ok())
        .and_then(rustix::process::Pid::from_raw);
    let answer = tokio::time::timeout(ASK_WAIT, talk(&mut child, &first, heard))
        .await
        .unwrap_or(Err(Fault::Slow));
    if let Some(group) = group {
        // A group that already ended is what this wants: the error is not a fault.
        let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
    }
    let _ = child.wait().await;
    answer
}

/// Writes the first line and reads the lines until the answer.
async fn talk<T>(
    child: &mut tokio::process::Child,
    first: &str,
    mut heard: impl FnMut(&Value) -> Heard<T>,
) -> Result<T, Fault> {
    let mut input = child.stdin.take().ok_or(Fault::NotStarted)?;
    let output = child.stdout.take().ok_or(Fault::NotStarted)?;
    let mut lines = tokio::io::BufReader::new(output);
    let most_line = u64::try_from(MOST_LINE).unwrap_or(u64::MAX);
    write_line(&mut input, first).await.ok_or(Fault::Ended)?;
    let mut line = String::new();
    for _ in 0..MOST_LINES {
        line.clear();
        // A line longer than the limit is never held whole.
        let mut one = (&mut lines).take(most_line + 1);
        match one.read_line(&mut line).await {
            Ok(0) | Err(_) => return Err(Fault::Ended),
            Ok(_) if line.len() > MOST_LINE => return Err(Fault::TooMuch),
            Ok(_) => {}
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match heard(&value) {
            Heard::Done(answer) => return Ok(answer),
            Heard::Write(more) => {
                for each in more {
                    write_line(&mut input, &each).await.ok_or(Fault::Ended)?;
                }
            }
            Heard::Nothing => {}
        }
    }
    Err(Fault::TooMuch)
}

async fn write_line(input: &mut tokio::process::ChildStdin, line: &str) -> Option<()> {
    input.write_all(line.as_bytes()).await.ok()?;
    input.write_all(b"\n").await.ok()?;
    input.flush().await.ok()
}

//! The agent threads of this computer, through the daemon's runner (TH13, THR-030, THR-049).
//! No MCP tool starts a thread or answers a request: an answer is a trust decision (THR-114).

use std::path::PathBuf;

use agent_threads::control::{
    Action, OVER_LIMIT, Outcome, Refusal, Reply, RunState, ThreadRow, WorkspaceKind,
};
use agent_threads::drive::{Effort, Mode, Program};
use uuid::Uuid;

use super::runner::{RunnerLink, refusal};

mod approve;
mod queue;
mod send;
mod set;
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::output::stream::{self, Reader};
use crate::output::{Cell, Table, Tone};
pub use approve::ApproveArgs;
pub use queue::QueueArgs;
pub use send::SendArgs;
pub use set::SetArgs;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum ProgramName {
    ClaudeCode,
    Codex,
}

impl ProgramName {
    #[must_use]
    pub const fn program(self) -> Program {
        match self {
            Self::ClaudeCode => Program::ClaudeCode,
            Self::Codex => Program::Codex,
        }
    }
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum ModeName {
    /// The program asks nothing (the default, R-308).
    FullAccess,
    /// The program changes files in the folder without asking and asks for the rest.
    AcceptEdits,
    /// The program asks before each change.
    Ask,
    /// The program plans and changes nothing until you accept its plan (Claude Code only).
    Plan,
}

impl ModeName {
    const fn mode(self) -> Mode {
        match self {
            Self::FullAccess => Mode::FullAccess,
            Self::AcceptEdits => Mode::AcceptEdits,
            Self::Ask => Mode::Ask,
            Self::Plan => Mode::Plan,
        }
    }
}

#[derive(Debug, clap::Args)]
pub struct StartArgs {
    #[arg(long, value_enum)]
    program: ProgramName,
    /// The folder the agent works in: an absolute folder of this computer.
    #[arg(long)]
    folder: PathBuf,
    #[arg(long, value_enum, default_value = "full-access")]
    mode: ModeName,
    #[arg(long)]
    model: Option<String>,
    #[arg(long, value_parser = parse_effort)]
    effort: Option<Effort>,
    /// A speed of the model other than the standard one, from `anima agents models`.
    #[arg(long)]
    speed: Option<String>,
    /// A different copy of the program than the first on the PATH of your login, such as the
    /// copy of an application. The runner takes only a copy that it finds itself.
    #[arg(long)]
    program_path: Option<PathBuf>,
    /// Run the thread in a new worktree of its own of the repository at `--folder`, on a new
    /// branch; your checkout does not change (TH116).
    #[arg(long)]
    worktree: bool,
    /// The branch or commit that the worktree starts from (the current one when not named).
    #[arg(long, requires = "worktree")]
    base: Option<String>,
    /// The first prompt.
    prompt: String,
}

#[derive(Debug, clap::Args)]
pub struct ThreadIdArgs {
    thread: Uuid,
}

#[derive(Debug, clap::Args)]
pub struct WatchArgs {
    /// Only the updates of this thread.
    thread: Option<Uuid>,
}

fn parse_effort(text: &str) -> Result<Effort, String> {
    serde_json::from_value(serde_json::Value::from(text))
        .map_err(|_| "none, minimal, low, medium, high, xhigh, max or ultra".to_owned())
}

/// Gives a result that is not a refusal.
pub(super) fn accepted(outcome: Outcome) -> Result<Outcome, Failure> {
    match outcome {
        Outcome::Refused { why } => Err(refusal(&why)),
        other => Ok(other),
    }
}

/// Said when the project files of the folder could not be checked before a start.
const NOT_CHECKED: &str = "The project files of this folder could not be checked: they can make Claude Code run commands with no question.";

impl StartArgs {
    /// Starts a thread. At the soft limit the person is asked whether to raise it (THR-114).
    ///
    /// # Errors
    /// [`Failure`] for a refusal of the runner, a no at the limit, or a runner that is not there.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let mut link = RunnerLink::connect().await?;
        let folder = std::path::absolute(&self.folder).unwrap_or(self.folder);
        // A failed folder check is said and does not stop the start (THR-046, OD-THR21).
        let checked = link
            .call(Action::Folder {
                folder: folder.clone(),
            })
            .await;
        match checked.map(accepted) {
            Ok(Ok(Outcome::Folder { notice })) if notice.is_empty() => {}
            Ok(Ok(Outcome::Folder { notice })) => invocation.output.message(&notice.join("\n")),
            _ => invocation.output.message(NOT_CHECKED),
        }
        let action = Action::Start {
            program: self.program.program(),
            folder,
            mode: self.mode.mode(),
            model: self.model,
            effort: self.effort,
            speed: self.speed,
            prompt: self.prompt,
            program_path: self
                .program_path
                .map(|path| std::path::absolute(&path).unwrap_or(path)),
            draft: None,
            attachments: Vec::new(),
            worktree: self.worktree,
            base: self.base,
        };
        loop {
            match link.call(action.clone()).await? {
                Outcome::Started { thread, .. } => {
                    return invocation
                        .output
                        .data(&thread, |thread| one(&thread.to_string()));
                }
                Outcome::Refused {
                    why: Refusal::OverLimit { .. },
                } if invocation.prompter.interactive() => {
                    invocation.prompter.confirm(OVER_LIMIT)?;
                    accepted(link.call(Action::RaiseLimit).await?)?;
                }
                other => {
                    accepted(other)?;
                    return Ok(());
                }
            }
        }
    }
}

fn one(text: &str) -> Table {
    let mut table = Table::new(vec!["THREAD"]);
    table.row(vec![Cell::new(text)]);
    table
}

/// Sends one call whose answer is only that it was done.
async fn simple(invocation: &Invocation, action: Action, done: &str) -> Result<(), Failure> {
    let mut link = RunnerLink::connect().await?;
    let stopped = match &action {
        Action::Stop { thread } => Some(*thread),
        _ => None,
    };
    if let (Outcome::Stopped { all_ended: false }, Some(thread)) =
        (accepted(link.call(action).await?)?, stopped)
    {
        invocation.output.message(&not_proved(thread));
    } else {
        invocation.output.message(done);
    }
    Ok(())
}

/// What the person does when a stop could not prove that each process of the thread ended
/// (THR-049): the holder sweeps it again at its next start; until then, the person can find what
/// still runs by the mark of the thread.
fn not_proved(thread: Uuid) -> String {
    format!(
        "The program ended, but a process below it could not be proved to have ended. The next start of the holder looks for it again. To look now: `ps axeww | grep -F AGENT_THREAD={thread} | grep -v grep` shows a process of the thread, and `kill` with its process ID ends it. On a Mac this cannot show a program signed by Apple, such as /bin/sh, because macOS hides its environment."
    )
}

/// `anima agents interrupt`: the running turn stops; the program stays.
///
/// # Errors
/// [`Failure`] for a refusal of the runner.
pub async fn interrupt(args: ThreadIdArgs, invocation: &Invocation) -> Result<(), Failure> {
    simple(
        invocation,
        Action::Interrupt {
            thread: args.thread,
        },
        "The turn stopped.",
    )
    .await
}

/// `anima agents compact`: Codex compacts the context of the thread (TH117).
///
/// # Errors
/// [`Failure`] for a refusal of the runner.
pub async fn compact(args: ThreadIdArgs, invocation: &Invocation) -> Result<(), Failure> {
    simple(
        invocation,
        Action::Compact {
            thread: args.thread,
        },
        "Codex is compacting the context of the thread.",
    )
    .await
}

/// `anima agents stop`: the program and each process below it end.
///
/// # Errors
/// [`Failure`] for a refusal of the runner.
pub async fn stop(args: ThreadIdArgs, invocation: &Invocation) -> Result<(), Failure> {
    simple(
        invocation,
        Action::Stop {
            thread: args.thread,
        },
        "The program and its processes ended.",
    )
    .await
}

/// `anima agents raise-limit`: the person raises the soft limit one step (THR-114).
///
/// # Errors
/// [`Failure::Declined`] for a no; a refusal of the runner.
pub async fn raise_limit(invocation: &Invocation) -> Result<(), Failure> {
    invocation.prompter.confirm(OVER_LIMIT)?;
    let mut link = RunnerLink::connect().await?;
    if let Outcome::Limits { limits } = accepted(link.call(Action::RaiseLimit).await?)? {
        invocation.output.message(&format!(
            "The limits are now {} deep, {} children of one thread and {} threads at one time.",
            limits.depth, limits.children, limits.running
        ));
    }
    Ok(())
}

/// `anima agents running`: the threads the runner started, with their state.
///
/// # Errors
/// [`Failure`] when the runner is not there.
pub async fn running(invocation: &Invocation) -> Result<(), Failure> {
    let mut link = RunnerLink::connect().await?;
    let Outcome::Threads { threads, holder } = accepted(link.call(Action::List).await?)? else {
        return Ok(());
    };
    invocation.output.data(&threads, |threads| table(threads))?;
    // Stop reasons and the holder go below the table (TH97, TH98).
    let day_ago = chrono::Utc::now() - chrono::Duration::days(1);
    for row in threads.iter().filter(|row| row.started > day_ago) {
        if let (RunState::Stopped, Some(ended)) = (row.state, &row.ended) {
            invocation.output.message(&format!("{}: {ended}", row.id));
        }
    }
    if let Some(holder) = holder {
        invocation.output.message(&holder);
    }
    Ok(())
}

fn table(rows: &[ThreadRow]) -> Table {
    let mut table = Table::new(vec![
        "STATE", "PROGRAM", "TITLE", "FOLDER", "CONTEXT", "THREAD",
    ]);
    for row in rows {
        let state = serde_json::to_value(row.state).ok();
        table.row(vec![
            Cell::toned(
                state.as_ref().and_then(|v| v.as_str()).unwrap_or("-"),
                Tone::Muted,
            ),
            Cell::new(super::threads::shown::program_words(program_key(
                row.program,
            ))),
            Cell::new(&row.title),
            Cell::new(&folder_words(row)),
            Cell::new(&context_words(row)),
            Cell::new(&row.id.to_string()),
        ]);
    }
    table
}

/// The folder of a row, with "(worktree)" when it is a linked worktree of Git (TH112).
fn folder_words(row: &ThreadRow) -> String {
    let folder = row.folder.display();
    match &row.workspace {
        Some(workspace) if workspace.kind == WorkspaceKind::Worktree => {
            format!("{folder} (worktree)")
        }
        _ => folder.to_string(),
    }
}

/// How full the context window is, as the program last said it (TH112); "-" when it did not.
fn context_words(row: &ThreadRow) -> String {
    match row
        .usage
        .as_ref()
        .and_then(|usage| usage.context.zip(usage.window))
    {
        Some((context, window)) if window > 0 => {
            format!("{}%", context.saturating_mul(100) / window)
        }
        _ => "-".to_owned(),
    }
}

pub(super) const fn program_key(program: Program) -> &'static str {
    match program {
        Program::ClaudeCode => "claude_code",
        Program::Codex => "codex",
    }
}

/// `anima agents watch`: the updates of the threads as they happen, one line each.
///
/// # Errors
/// [`Failure`] when the runner is not there or standard output fails.
pub async fn watch(args: WatchArgs, invocation: &Invocation) -> Result<(), Failure> {
    let mut link = RunnerLink::connect().await?;
    link.send(Action::Watch).await?;
    while let Some(reply) = link.next().await? {
        let Reply::Update(update) = reply else {
            continue;
        };
        if args.thread.is_some_and(|thread| thread != update.thread) {
            continue;
        }
        let Some(text) = super::watch_text::text(&update.change) else {
            continue;
        };
        let thread = update.thread.to_string();
        let shown = format!("{} {text}", &thread[..8]);
        if stream::emit(invocation.output.json(), &*update, |_| shown)? == Reader::Gone {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

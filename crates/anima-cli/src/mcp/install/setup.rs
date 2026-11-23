//! One agent on this computer: whether it is there, what its entry is, what a change would touch,
//! and the change itself (MCP-027 to MCP-036). This is the edge: it reads files and runs the
//! agents' own programs; `agents.rs` and `jsonc.rs` decide what to write.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::access::Others;
use super::agents::{Agent, Check, NAME, State, Way, arguments, entry, file};
use super::files::{self, FileError, copy_path};
use super::jsonc::{self, JsoncError};

mod by_hand;
mod here;
pub use here::Here;
mod program;
#[cfg(unix)]
mod script;
#[cfg(windows)]
mod windows;
pub use program::{Checked, CommandError, check_command};
use program::{Found, fixed_program};
mod run;
use run::{Ran, run};

/// Whether the agent is on this computer (MCP-027).
#[must_use]
pub fn present(agent: &Agent, here: &Here) -> bool {
    #[cfg(windows)]
    if matches!(agent.id, "claude-desktop" | "zed") {
        return windows::file(agent.id, here.roaming.as_deref())
            .and_then(|path| path.parent().map(Path::is_dir))
            .unwrap_or(false);
    }
    agent
        .programs
        .iter()
        .any(|name| program::program(here, name).is_some())
        || agent
            .folders
            .iter()
            .any(|folder| here.home.join(folder).is_dir())
}

fn file_of(agent: &Agent, here: &Here) -> Option<PathBuf> {
    #[cfg(windows)]
    if matches!(agent.id, "claude-desktop" | "zed") {
        return windows::file(agent.id, here.roaming.as_deref());
    }
    match agent.way {
        Way::File { place, .. } => Some(file(place, &here.home, here.macos, Path::is_file)),
        Way::Own { .. } => None,
    }
}

/// What the agent's entry is now (MCP-027). A file way reads the file; an own way asks the program.
#[must_use]
pub fn state(agent: &Agent, here: &Here, anima: &Path) -> State {
    if !present(agent, here) {
        return State::NotInstalled;
    }
    match agent.way {
        Way::File { keys, shape, .. } => {
            let Some(path) = file_of(agent, here) else {
                return State::Unreadable;
            };
            let Ok(text) = files::read(here.root(&path), &path) else {
                return State::Unreadable;
            };
            match jsonc::member(text.as_deref().unwrap_or(""), keys, NAME) {
                Err(_) => State::Unreadable,
                Ok(None) => State::NotSetUp,
                // A value with a comment in it does not parse: a person wrote it, so it is not ours.
                Ok(Some(found))
                    if serde_json::from_str::<serde_json::Value>(found)
                        .is_ok_and(|value| value == entry(shape, anima)) =>
                {
                    State::SetUp
                }
                Ok(Some(_)) => State::Different,
            }
        }
        Way::Own { program, check, .. } => {
            let (Check::Get(template) | Check::List(template)) = check;
            let (success, stdout) = match run(here, program, &arguments(template, anima)) {
                Ran::Done { success, stdout } => (success, stdout),
                Ran::Missing => return State::NotSetUp,
                Ran::Failed | Ran::Replaceable { .. } => return State::Unreadable,
            };
            // The path as one whole word of the output, not as a part of a longer word: a
            // server at `<anima>-other` is not this one. What else the entry holds is not read.
            let output_text = String::from_utf8_lossy(&stdout);
            let path = anima.to_string_lossy();
            let words = |text: &str, word: &str| {
                text.split(|character: char| {
                    character.is_whitespace() || matches!(character, '"' | '\'' | ',' | '[' | ']')
                })
                .any(|part| part == word)
            };
            match check {
                Check::Get(_) if !success => State::NotSetUp,
                Check::Get(_) | Check::List(_) if words(&output_text, &path) => State::SetUp,
                Check::Get(_) => State::Different,
                // A list that names a server `anima` with a different program: not ours.
                Check::List(_)
                    if words(&output_text, NAME) || words(&output_text, &format!("{NAME}:")) =>
                {
                    State::Different
                }
                Check::List(_) => State::NotSetUp,
            }
        }
    }
}

/// What a change touches, shown before it is done (MCP-029).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Step {
    /// Writes `file`; `copy` is the copy of the file as it was that this change makes, if any.
    Write {
        file: PathBuf,
        copy: Option<PathBuf>,
    },
    /// Runs `program` with `arguments`.
    Run {
        program: PathBuf,
        arguments: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Install,
    Uninstall,
}

/// Why one agent was not changed. No variant carries text of a file (LOG-007). When a different
/// user can change the agent's program or file, the error names who and what the person can run
/// or add themselves (R-442).
#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("its file was refused: {0}")]
    File(#[from] FileError),
    #[error("its file was refused: {others}. {by_hand}")]
    FileShared { others: Others, by_hand: String },
    #[error("its file was refused: {0}")]
    Format(#[from] JsoncError),
    #[error("its program is not on PATH")]
    NoProgram,
    #[error("a different user can change its program: {others}. {by_hand}")]
    ProgramReplaceable { others: Others, by_hand: String },
    #[error("its program ended with a failure")]
    ProgramFailed,
}

/// The refusal of an agent's own program that a different user can change, with the command
/// that the person can run themselves.
fn replaceable(action: Action, program: &Path, arguments: &[String], others: Others) -> SetupError {
    SetupError::ProgramReplaceable {
        others,
        by_hand: by_hand::command(action, program, arguments),
    }
}

/// The refusal of an agent's file. When a different user can change it, the refusal names who and
/// the member that the person can add or take out themselves.
fn file_refused(
    error: FileError,
    agent: &Agent,
    path: &Path,
    (action, anima): (Action, &Path),
) -> SetupError {
    match (error, agent.way) {
        (FileError::Shared(others), Way::File { keys, shape, .. }) => SetupError::FileShared {
            others,
            by_hand: by_hand::member(
                action,
                path,
                keys,
                &serde_json::to_string(&entry(shape, anima)).unwrap_or_default(),
            ),
        },
        (error, _) => SetupError::File(error),
    }
}

/// The steps that `action` would take for `agent` in `state`, in order (MCP-029). It reads only.
///
/// # Errors
/// [`SetupError::NoProgram`] when an own way has no program on `PATH`; the refusals of a file
/// ([`SetupError::File`], [`SetupError::FileShared`]) or a program that a different user can
/// change ([`SetupError::ProgramReplaceable`]).
pub fn steps(
    agent: &Agent,
    here: &Here,
    anima: &Path,
    wanted: (Action, State),
) -> Result<Vec<Step>, SetupError> {
    let (action, state) = wanted;
    match agent.way {
        Way::File { .. } => {
            let file = file_of(agent, here).ok_or(SetupError::NoProgram)?;
            // A file that a different user can change: the list says who, and what to do by hand.
            files::read(here.root(&file), &file)
                .map_err(|error| file_refused(error, agent, &file, (action, anima)))?;
            // `symlink_metadata`: a link at the name of the copy is not a copy.
            let copy = (file.exists() && fs::symlink_metadata(copy_path(&file)).is_err())
                .then(|| copy_path(&file));
            Ok(vec![Step::Write { file, copy }])
        }
        Way::Own {
            program: name,
            add,
            remove,
            ..
        } => {
            let program = match fixed_program(here, name) {
                Found::Fixed(path) => path,
                Found::Replaceable { program, others } => {
                    let template = if action == Action::Install {
                        add
                    } else {
                        remove
                    };
                    let arguments = arguments(template, anima);
                    return Err(replaceable(action, &program, &arguments, others));
                }
                Found::Missing => return Err(SetupError::NoProgram),
            };
            let run = |template: &[&str]| Step::Run {
                program: program.clone(),
                arguments: arguments(template, anima),
            };
            Ok(match (action, state) {
                // The agents refuse a name that is there: the entry of a person is taken out first.
                (Action::Install, State::Different) => vec![run(remove), run(add)],
                (Action::Install, _) => vec![run(add)],
                (Action::Uninstall, _) => vec![run(remove)],
            })
        }
    }
}

/// Does `action` for `agent` (MCP-028, MCP-036).
///
/// # Errors
/// [`SetupError`] when the file is refused or the agent's program fails; nothing is then changed.
pub fn apply(agent: &Agent, here: &Here, anima: &Path, action: Action) -> Result<(), SetupError> {
    match agent.way {
        Way::File { keys, shape, .. } => {
            let path = file_of(agent, here).ok_or(SetupError::NoProgram)?;
            let refused = |error| file_refused(error, agent, &path, (action, anima));
            let before = files::read(here.root(&path), &path).map_err(refused)?;
            let text = before.as_deref().unwrap_or("");
            let after = match action {
                Action::Install => {
                    let value = serde_json::to_string_pretty(&entry(shape, anima))
                        .map_err(|_| JsoncError::Malformed)?;
                    Some(jsonc::set(text, keys, NAME, &value)?)
                }
                Action::Uninstall => jsonc::remove(text, keys, NAME)?,
            };
            match after {
                Some(after) if after != text => {
                    files::replace(here.root(&path), &path, before.as_deref(), &after)
                        .map_err(refused)
                }
                _ => Ok(()),
            }
        }
        Way::Own {
            program,
            add,
            remove,
            ..
        } => {
            let state = state(agent, here, anima);
            for template in if action == Action::Install && state == State::Different {
                vec![remove, add]
            } else if action == Action::Install {
                vec![add]
            } else {
                vec![remove]
            } {
                let arguments = arguments(template, anima);
                match run(here, program, &arguments) {
                    Ran::Done { success: true, .. } => {}
                    Ran::Done { .. } | Ran::Failed => return Err(SetupError::ProgramFailed),
                    Ran::Replaceable { program, others } => {
                        return Err(replaceable(action, &program, &arguments, others));
                    }
                    Ran::Missing => return Err(SetupError::NoProgram),
                }
            }
            Ok(())
        }
    }
}

#[cfg(all(test, unix))]
mod tests;

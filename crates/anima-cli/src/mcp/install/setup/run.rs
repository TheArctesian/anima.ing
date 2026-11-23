//! Runs an agent's own program (MCP-035): fixed arguments, no shell, the home folder, the `PATH`
//! that was checked, a time limit.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use super::super::access::Others;
use super::super::limited;
use super::Here;
use super::program::{Found, fixed_program};

/// The longest time an agent's own program may take; it is then stopped and counts as failed.
const RUN_LIMIT: Duration = Duration::from_secs(20);

/// The most of the output of an agent's own program that is read.
const OUTPUT_LIMIT: u64 = 1 << 20;

/// What an agent's own program did: its exit and what it wrote, or why it did not run.
pub(super) enum Ran {
    Done {
        success: bool,
        stdout: Vec<u8>,
    },
    /// It ran past [`RUN_LIMIT`] and was stopped, or could not be started.
    Failed,
    /// A different user can replace it (MCP-033): it is not run.
    Replaceable {
        program: PathBuf,
        others: Others,
    },
    Missing,
}

/// Runs an agent's own program with fixed arguments and no shell (MCP-035), for at most
/// [`RUN_LIMIT`].
pub(super) fn run(here: &Here, name: &str, arguments: &[String]) -> Ran {
    let program = match fixed_program(here, name) {
        Found::Fixed(path) => path,
        Found::Replaceable { program, others } => return Ran::Replaceable { program, others },
        Found::Missing => return Ran::Missing,
    };
    // The `PATH` that the check read: a script that starts `env node` finds the `node` that was
    // checked, also when the caller (the desktop application) has a different `PATH`.
    let Ok(path) = std::env::join_paths(&here.path) else {
        return Ran::Failed;
    };
    let mut command = Command::new(program);
    // In the home folder: an agent must not read the project configuration of the folder the
    // person happens to be in.
    command
        .args(arguments)
        .current_dir(&here.home)
        .env("PATH", path);
    limited::run(command, RUN_LIMIT, OUTPUT_LIMIT).map_or(Ran::Failed, |output| Ran::Done {
        success: output.success,
        stdout: output.stdout,
    })
}

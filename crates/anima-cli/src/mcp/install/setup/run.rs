//! Runs an agent's own program (MCP-035): fixed arguments, no shell, the home folder, the folders
//! of `PATH` that the check accepted, a time limit.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use super::super::access::Others;
use super::super::limited;
use super::Here;
use super::program::{Found, fixed_program};

const RUN_LIMIT: Duration = Duration::from_secs(20);

const OUTPUT_LIMIT: u64 = 1 << 20;

pub(super) enum Ran {
    Done {
        success: bool,
        stdout: Vec<u8>,
    },
    Failed,
    /// A different user can replace it, so it is not run (MCP-033).
    Replaceable {
        program: PathBuf,
        others: Others,
    },
    Missing,
}

pub(super) fn run(here: &Here, name: &str, arguments: &[String]) -> Ran {
    let program = match fixed_program(here, name) {
        Found::Fixed(path) => path,
        Found::Replaceable { program, others } => return Ran::Replaceable { program, others },
        Found::Missing => return Ran::Missing,
    };
    // Only the checked `PATH` folders, so `env node` finds the checked `node` whatever the caller's `PATH`.
    #[cfg(unix)]
    let folders = super::script::folders(here);
    #[cfg(windows)]
    let folders = here.path.clone();
    // With no folder, the system would look in the folder the program is in at the time.
    if folders.is_empty() {
        return Ran::Failed;
    }
    let Ok(path) = std::env::join_paths(&folders) else {
        return Ran::Failed;
    };
    let mut command = Command::new(program);
    // The home folder, so an agent reads no project configuration of the person's current folder.
    command
        .args(arguments)
        .current_dir(&here.home)
        .env("PATH", path);
    limited::run(command, RUN_LIMIT, OUTPUT_LIMIT).map_or(Ran::Failed, |output| Ran::Done {
        success: output.success,
        stdout: output.stdout,
    })
}

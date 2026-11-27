//! Local thread runner and holder entry points.
use std::process::ExitCode;

#[cfg(unix)]
use super::{failure, stop};

#[cfg(unix)]
pub(super) async fn threads(daemon_dir: std::path::PathBuf) -> ExitCode {
    use anima_daemon::threads::runner;
    let program = match std::env::current_exe() {
        Ok(program) => program,
        Err(error) => return failure(&error),
    };
    let daemon = runner::login::of_the_daemon();
    match runner::run(&daemon_dir, &program, daemon, stop()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => failure(&error),
    }
}

#[cfg(unix)]
pub(super) fn serves_a_person(daemon_dir: &std::path::Path) -> bool {
    anima_daemon::threads::runner::serves_a_person(daemon_dir)
}

#[cfg(not(unix))]
pub(super) const fn serves_a_person(_: &std::path::Path) -> bool {
    false
}

#[cfg(not(unix))]
pub(super) async fn threads(_: std::path::PathBuf) -> ExitCode {
    ExitCode::FAILURE
}

/// THR-113.
#[cfg(unix)]
pub(super) async fn holder(folder: &std::path::Path, limit: usize) -> ExitCode {
    match anima_daemon::threads::holder::hold(folder, limit).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => failure(&error),
    }
}

#[cfg(not(unix))]
pub(super) async fn holder(_: &std::path::Path, _: usize) -> ExitCode {
    ExitCode::FAILURE
}

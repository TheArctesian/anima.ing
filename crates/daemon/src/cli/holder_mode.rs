#[cfg(unix)]
use super::failure;
use super::{Command, ExitCode};

pub(super) async fn run(command: &Command) -> Option<ExitCode> {
    match command {
        Command::Holder {
            folder,
            limit,
            channel,
        } => Some(holder(folder, *limit, *channel).await),
        #[cfg(unix)]
        Command::HolderPublicRoot(dir) => Some(
            match anima_daemon::threads::holder_channel::write_public_root(dir) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::FAILURE,
            },
        ),
        _ => None,
    }
}

/// THR-113.
#[cfg(unix)]
async fn holder(folder: &std::path::Path, limit: usize, channel: bool) -> ExitCode {
    match anima_daemon::threads::holder::hold_with_channel(folder, limit, channel).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => failure(&error),
    }
}

#[cfg(not(unix))]
#[expect(
    clippy::unused_async,
    reason = "the holder runs only on Unix; the caller awaits this on each system"
)]
async fn holder(_: &std::path::Path, _: usize, _: bool) -> ExitCode {
    ExitCode::FAILURE
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub(super) fn prepared_args() -> std::io::Result<Option<Vec<String>>> {
    let mut args = std::env::args().skip(1).collect();
    if anima_daemon::linux_role::enter(&mut args)? {
        return Ok(None);
    }
    Ok(Some(args))
}

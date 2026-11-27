//! Serialises credential changes across the app's CLI children and ordinary CLI processes.
use super::device::ended;
use crate::{failure::Failure, private_file};
use std::{
    fs::{File, TryLockError},
    io,
    path::Path,
    time::{Duration, Instant},
};

const WAIT_LIMIT: Duration = Duration::from_secs(40);
const RETRY_INTERVAL: Duration = Duration::from_millis(50);
const ERROR_SHARING_VIOLATION: i32 = 32;

pub(super) async fn acquire(dir: &Path) -> Result<File, Failure> {
    let path = dir.join("auth.lock");
    match private_file::write_new(dir, "auth.lock", b"") {
        Ok(()) => {}
        Err(private_file::PrivateFileError::Io(error))
            if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(private_file::PrivateFileError::Io(error)) => {
            return Err(io_failure(
                "The credential lock could not be created",
                &error,
            ));
        }
        Err(_) => return Err(ended("The credential lock could not be created.")),
    }

    let deadline = Instant::now() + WAIT_LIMIT;
    let file = open_lock(&path, deadline)
        .await
        .map_err(|error| io_failure("The credential lock could not be opened", &error))?;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if wait_to_retry(deadline).await => {}
            Err(TryLockError::WouldBlock) => {
                return Err(ended(
                    "Another sign-in is still finishing. Try again shortly.",
                ));
            }
            Err(TryLockError::Error(error)) => {
                return Err(io_failure(
                    "The credential lock could not be acquired",
                    &error,
                ));
            }
        }
    }
}

async fn open_lock(path: &Path, deadline: Instant) -> io::Result<File> {
    loop {
        let attempt = open_file(path);
        if !matches!(&attempt, Err(error) if cfg!(windows) && error.raw_os_error() == Some(ERROR_SHARING_VIOLATION))
            || !wait_to_retry(deadline).await
        {
            return attempt;
        }
    }
}

#[cfg(windows)]
fn open_file(path: &Path) -> io::Result<File> {
    anima_windows::files::read(path, true)
}

#[cfg(not(windows))]
fn open_file(path: &Path) -> io::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed());
    }
    options.open(path)
}

async fn wait_to_retry(deadline: Instant) -> bool {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return false;
    }
    tokio::time::sleep(remaining.min(RETRY_INTERVAL)).await;
    Instant::now() < deadline
}

fn io_failure(message: &str, error: &io::Error) -> Failure {
    if cfg!(windows)
        && let Some(code) = error.raw_os_error()
    {
        return ended(&format!("{message} (Windows error {code})."));
    }
    ended(&format!("{message}."))
}

#[cfg(all(test, windows))]
mod tests;

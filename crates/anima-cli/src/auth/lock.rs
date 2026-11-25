//! Serialises credential changes across the app's CLI children and ordinary CLI processes.
use super::device::ended;
use crate::{failure::Failure, private_file};
use std::{
    fs::{File, OpenOptions},
    path::Path,
    time::{Duration, Instant},
};

pub(super) async fn acquire(dir: &Path) -> Result<File, Failure> {
    let path = dir.join("auth.lock");
    match private_file::write_new(dir, "auth.lock", b"") {
        Ok(()) => {}
        Err(private_file::PrivateFileError::Io(error))
            if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(ended("The credential lock could not be created.")),
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed());
    }
    let file = options
        .open(path)
        .map_err(|_| ended("The credential lock could not be opened."))?;
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            _ => {
                return Err(ended(
                    "Another sign-in is still finishing. Try again shortly.",
                ));
            }
        }
    }
}

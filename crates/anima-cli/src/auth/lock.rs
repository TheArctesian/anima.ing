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
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        // On Windows a second process can find the file still held by the one that creates it.
        let opened = options.open(&path);
        let waits = Instant::now() < deadline;
        match opened.map(|file| (file.try_lock(), file)) {
            Ok((Ok(()), file)) => return Ok(file),
            Ok((Err(std::fs::TryLockError::WouldBlock), _)) if waits => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(_) if waits && cfg!(windows) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(_) => return Err(ended("The credential lock could not be opened.")),
            Ok(_) => {
                return Err(ended(
                    "Another sign-in is still finishing. Try again shortly.",
                ));
            }
        }
    }
}

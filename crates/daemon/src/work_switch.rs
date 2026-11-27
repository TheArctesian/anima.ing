//! The switch at the machine (USE-004; K10 Section 3, SF-007): a private file in the daemon
//! folder. Only the owner of the machine sets it, with `anima-daemon work on` or `off`; the hub
//! cannot write the folder, and an update of the daemon does not change the file.

use std::path::Path;

use crate::config::home::{Home, HomeError};
use crate::config::private_file::{self, PrivateFileError};

pub const FILE: &str = "work-switch";

const ON: &[u8] = b"on\n";
const OFF: &[u8] = b"off\n";

/// More than each text that [`set`] writes, so a longer file does not read.
const LIMIT: usize = 16;

/// The switch is on when the file is not there (USE-004: on by default), and off when the file
/// says `off` or holds anything that [`set`] does not write.
///
/// # Errors
/// [`PrivateFileError`] when the file is there and is not the private file of this user.
pub fn is_on(dir: &Path) -> Result<bool, PrivateFileError> {
    Ok(private_file::read_within(&dir.join(FILE), LIMIT)?.is_none_or(|text| text.as_slice() == ON))
}

/// # Errors
/// [`PrivateFileError`] when the file cannot be written; the old file then stays.
pub fn set(dir: &Path, on: bool) -> Result<(), PrivateFileError> {
    private_file::write(dir, FILE, if on { ON } else { OFF })
}

#[derive(Debug, thiserror::Error)]
pub enum SwitchError {
    #[error("{0}")]
    Home(#[from] HomeError),
    #[error("{0}")]
    File(#[from] PrivateFileError),
}

/// `anima-daemon work on` or `off`: sets the switch, and says what it means. The running daemon
/// reads the switch before each claim, so it needs no restart.
///
/// # Errors
/// [`SwitchError`] when the daemon folder or the file cannot be written.
pub fn turn_and_say(home: &Home, on: bool) -> Result<(), SwitchError> {
    home.ensure()?;
    set(home.dir(), on)?;
    if on {
        eprintln!("This machine takes work from the hub.");
    } else {
        eprintln!(
            "This machine takes no work from the hub. Running servers go on; the hub can still stop them."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_004_the_switch_is_on_by_default_and_keeps_what_the_owner_set() {
        let dir = tempfile::tempdir().expect("a folder");
        assert!(is_on(dir.path()).expect("reads"), "on with no file");
        set(dir.path(), false).expect("off");
        assert!(!is_on(dir.path()).expect("reads"));
        set(dir.path(), true).expect("on");
        assert!(is_on(dir.path()).expect("reads"));
    }

    #[test]
    fn use_004_a_file_that_set_did_not_write_is_off() {
        let dir = tempfile::tempdir().expect("a folder");
        for text in ["ON\n", "on", "yes\n", "on\non\n"] {
            private_file::write(dir.path(), FILE, text.as_bytes()).expect("written");
            assert!(!is_on(dir.path()).expect("reads"), "{text:?}");
        }
        private_file::write(dir.path(), FILE, &[b'\n'; 17]).expect("written");
        assert!(is_on(dir.path()).is_err(), "too long to read");
    }

    #[cfg(unix)]
    #[test]
    fn sf_007_a_switch_file_that_another_can_write_is_an_error() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().expect("a folder");
        set(dir.path(), true).expect("on");
        std::fs::set_permissions(
            dir.path().join(FILE),
            std::fs::Permissions::from_mode(0o666),
        )
        .expect("mode");
        assert!(matches!(is_on(dir.path()), Err(PrivateFileError::Exposed)));
    }
}

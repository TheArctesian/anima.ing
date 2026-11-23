//! Takes the daemon off this computer (HMD-056): the daemon removes its own unit and token, since a Keychain item opens without a question only for the program that made it.
//! The shell then deletes the copy that it made.

use std::{ffi::OsString, fs, io, path::Path, sync::atomic::AtomicBool, time::Duration};

use super::{
    DaemonError,
    run::{self, Run},
};

/// `uninstall` removes the user unit or `LaunchAgent`, never a system unit, which needs root.
pub const UNINSTALL_ARGS: [&str; 1] = ["uninstall"];

pub struct Leave<'a> {
    pub copy: &'a Path,
    pub carried: Option<&'a Path>,
    pub env: &'a [(&'static str, OsString)],
    pub deadline: Duration,
    pub stop: &'a AtomicBool,
}

/// A second removal ends as the first. True when the copy was here: the hub may list this daemon.
///
/// # Errors
/// [`DaemonError::Uninstall`], [`DaemonError::TimedOut`] or [`DaemonError::Failed`] leave the copy for a later removal; [`DaemonError::Remove`] when it cannot be deleted.
pub fn remove_here(leave: &Leave<'_>) -> Result<bool, DaemonError> {
    let copied = leave.copy.is_file();
    let program = copied
        .then_some(leave.copy)
        .or(leave.carried.filter(|carried| carried.is_file()));
    let Some(program) = program else {
        return Ok(false);
    };
    let uninstall = Run {
        program,
        args: &UNINSTALL_ARGS,
        input: None,
        env: leave.env,
        deadline: leave.deadline,
        stop: leave.stop,
    };
    run::run(&uninstall).map_err(|error| DaemonError::of_run(error, DaemonError::Uninstall))?;
    // A copy of before left by a cut update (HMD-055) goes too.
    #[cfg(unix)]
    delete(&super::update::previous(leave.copy))?;
    delete(leave.copy).map(|()| copied)
}

fn delete(file: &Path) -> Result<(), DaemonError> {
    match fs::remove_file(file) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(DaemonError::Remove),
    }
}

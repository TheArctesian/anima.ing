//! Replaces the installed daemon copy with the newer one the app carries; the token and the configuration stay (HMD-055).
//! The unit stops first, with the carried daemon's `stop`, since an older daemon has no such command.
//! The old copy moves aside by a rename, so an open copy and its macOS signature stay valid; the new one is renamed into place, never written into the old file.
//! No other process is signalled: the holder of the agent threads (THR-113) keeps running.
//! A copy that is not installed is never installed here (HMD-053).
//! An ad hoc build also replaces an older copy; the Keychain then asks the person once (R-437).
//! Only the carried daemon beside a running app whose seal verifies is placed, and the new daemon must answer with its version after its start.

use std::{
    ffi::OsString,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};

use tauri_plugin_log::log;

mod kept;

use super::{
    DaemonError, placement,
    run::{self, INSTALL_ARGS, Run, RunError},
    trust::User,
    version::{self, Decision, Version},
};

/// `stop`: the user unit stops; its file and the token stay (HMD-055).
pub const STOP_ARGS: [&str; 1] = ["stop"];

const VERSION_DEADLINE: Duration = Duration::from_secs(10);

pub struct Swap<'a> {
    pub copy: &'a Path,
    pub carried: &'a Path,
    pub user: User,
    pub env: &'a [(&'static str, OsString)],
    /// The deadline of the daemon's `stop` and of its `install`.
    pub restart: Duration,
    pub stop: &'a AtomicBool,
    /// No update from a bundle whose files changed after it was signed.
    pub sealed: bool,
    /// Checks the seal of the new copy under its fresh name, before its rename.
    pub signed: &'a dyn Fn(&Path) -> bool,
    pub healthy: &'a dyn Fn(Version) -> bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Updated {
    NoCopy,
    CarriedUnknown,
    Unchanged,
    KeptNewer,
    Replaced,
    NotSealed,
}

pub fn previous(copy: &Path) -> PathBuf {
    let mut name = copy.as_os_str().to_owned();
    name.push(".previous");
    PathBuf::from(name)
}

/// # Errors
/// A [`DaemonError`] for the first step that fails; except after [`DaemonError::Untrusted`] and [`DaemonError::Remove`], the copy of before is back in place.
pub fn update_here(swap: &Swap<'_>) -> Result<Updated, DaemonError> {
    let aside = previous(swap.copy);
    if fs::symlink_metadata(swap.copy).is_err() && placement::file_trusted(&aside, swap.user) {
        // An update cut between the two renames: the copy of before goes back to a trusted folder.
        folder_trusted(swap)?;
        put_back(swap.copy, &aside).map_err(|_| DaemonError::Copy)?;
    }
    if fs::symlink_metadata(swap.copy).is_err() {
        return Ok(Updated::NoCopy);
    }
    trusted(swap, &aside)?;
    let Some(carried) = read(swap, swap.carried) else {
        return Ok(Updated::CarriedUnknown);
    };
    let installed = read(swap, swap.copy);
    match version::decide(carried, installed) {
        Decision::Replace if !swap.sealed => Ok(Updated::NotSealed),
        Decision::Replace => replace(swap, &aside, carried, installed).map(|()| Updated::Replaced),
        Decision::Same => finish_earlier(swap, &aside, carried).map(|()| Updated::Unchanged),
        Decision::KeepNewer => finish_earlier(swap, &aside, carried).map(|()| Updated::KeptNewer),
    }
}

fn read(swap: &Swap<'_>, program: &Path) -> Option<Version> {
    version::read(program, swap.env, VERSION_DEADLINE, swap.stop)
}

/// Whether the installed `copy` is there and passes the checks of [`update_here`]; with no copy, no folder is made.
pub fn copy_trusted(copy: &Path, user: User) -> bool {
    fs::symlink_metadata(copy).is_ok()
        && copy
            .parent()
            .is_some_and(|folder| placement::trusted_folder(folder, user).is_ok())
        && placement::file_trusted(copy, user)
}

fn folder_trusted(swap: &Swap<'_>) -> Result<(), DaemonError> {
    let folder = swap.copy.parent().ok_or(DaemonError::Untrusted)?;
    placement::trusted_folder(folder, swap.user).map_err(DaemonError::of_placement)?;
    Ok(())
}

/// The checks of the hand install, before anything moves or runs; a link at the copy or the copy of before is refused, never followed.
fn trusted(swap: &Swap<'_>, aside: &Path) -> Result<(), DaemonError> {
    folder_trusted(swap)?;
    let aside_ok =
        fs::symlink_metadata(aside).is_err() || placement::file_trusted(aside, swap.user);
    if placement::file_trusted(swap.copy, swap.user) && aside_ok {
        return Ok(());
    }
    log::warn!(
        "the daemon update: the copy or the copy of before is a link or not a file of this user"
    );
    Err(DaemonError::Untrusted)
}

fn replace(
    swap: &Swap<'_>,
    aside: &Path,
    carried: Version,
    installed: Option<Version>,
) -> Result<(), DaemonError> {
    // A line before the first step, so that an update that the end of the app cuts leaves a trace.
    log::info!("the daemon update: the copy is older than the daemon of the app; replacing it");
    stop(swap)?;
    let failed = |error: DaemonError| {
        log::warn!(
            "the daemon update: the new copy was not put in place: {}",
            error.code()
        );
        let _ = restart(swap, &real(swap.copy), BEFORE);
        Err(error)
    };
    if fs::symlink_metadata(aside).is_err() && fs::rename(swap.copy, aside).is_err() {
        return failed(DaemonError::Copy);
    }
    let check = |part: &Path| (swap.signed)(part) && read(swap, part) == Some(carried);
    match placement::place_checked(swap.carried, swap.copy, swap.user, &check) {
        Ok(placed) => started(swap, &placed, aside, (carried, installed)),
        Err(error) => {
            if put_back(swap.copy, aside).is_err() {
                return Err(DaemonError::of_placement(error));
            }
            failed(DaemonError::of_placement(error))
        }
    }
}

/// A stop past its deadline may have stopped the unit, so the unit of the copy starts again.
fn stop(swap: &Swap<'_>) -> Result<(), DaemonError> {
    let Err(error) = stop_with(swap, swap.carried) else {
        return Ok(());
    };
    let failed = failed_step("stop", error, DaemonError::Failed);
    if error == RunError::TimedOut {
        let _ = restart(swap, &real(swap.copy), BEFORE);
    }
    Err(failed)
}

fn stop_with(swap: &Swap<'_>, program: &Path) -> Result<(), RunError> {
    let stop = Run {
        program,
        args: &STOP_ARGS,
        input: None,
        env: swap.env,
        deadline: swap.restart,
        stop: swap.stop,
    };
    run::run(&stop)
}

const BEFORE: &str = "install of the copy of before";
const NEW: &str = "install of the new copy";

/// Logs one line with the step name and the error code, never a path or an OS text.
fn failed_step(name: &str, error: RunError, failed: DaemonError) -> DaemonError {
    let error = DaemonError::of_run(error, failed);
    log::warn!(
        "the daemon update: the step `{name}` failed: {}",
        error.code()
    );
    error
}

fn real(copy: &Path) -> PathBuf {
    fs::canonicalize(copy).unwrap_or_else(|_| copy.to_owned())
}

/// Finishes an update cut after the new copy was put in place: its unit starts again.
fn finish_earlier(swap: &Swap<'_>, aside: &Path, carried: Version) -> Result<(), DaemonError> {
    if aside.is_file() {
        log::info!("the daemon update: an earlier update was cut; its unit starts again");
        let before = read(swap, aside);
        let running = read(swap, swap.copy).unwrap_or(carried);
        return started(swap, &real(swap.copy), aside, (running, before));
    }
    Ok(())
}

fn started(
    swap: &Swap<'_>,
    installed: &Path,
    aside: &Path,
    versions: (Version, Option<Version>),
) -> Result<(), DaemonError> {
    let begun = restart(swap, installed, NEW).and_then(|()| {
        if (swap.healthy)(versions.0) {
            return Ok(());
        }
        log::warn!("the daemon update: the new daemon did not answer after its start");
        let _ = stop_with(swap, installed);
        Err(DaemonError::Unhealthy)
    });
    if let Err(error) = begun {
        if put_back(swap.copy, aside).is_ok() {
            log::warn!("the daemon update: the copy of before is back; its unit starts again");
            // The new copy's error is reported; the next start of the app tries again.
            let _ = restart(swap, installed, BEFORE);
        } else {
            log::warn!("the daemon update: the copy of before could not be put back");
        }
        return Err(error);
    }
    kept::keep(swap.copy, aside, versions.1)
}

/// `install` writes the unit again and starts it with `program`; the token stays.
fn restart(swap: &Swap<'_>, program: &Path, name: &str) -> Result<(), DaemonError> {
    let install = Run {
        program,
        args: &INSTALL_ARGS,
        input: None,
        env: swap.env,
        deadline: swap.restart,
        stop: swap.stop,
    };
    run::run(&install).map_err(|error| failed_step(name, error, DaemonError::Install))
}

fn put_back(copy: &Path, aside: &Path) -> io::Result<()> {
    fs::rename(aside, copy)?;
    File::open(copy.parent().unwrap_or_else(|| Path::new("/")))?.sync_all()
}

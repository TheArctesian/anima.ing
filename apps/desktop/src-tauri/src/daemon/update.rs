//! Updates the installed copy of the daemon with the app (HMD-055): when the daemon that the app
//! carries is newer than the copy in `~/.local/bin` ([`super::version`]), the copy is replaced
//! and the unit starts again, through the daemon's own `install`, as at the first install. The
//! token and the configuration stay: there is no new enrolment.
//!
//! The order: first the unit stops (`anima-daemon stop` of the carried daemon, because a daemon
//! of before has no such command), so that no daemon of before runs, and none starts a new holder
//! of the agent threads, once the new copy is in place. Then the copy is moved aside to
//! `anima-daemon.previous` (a rename: the same file, so a program that still has it open keeps
//! it, and macOS still finds its code signature valid). The new copy is written beside it under a
//! fresh name, flushed and renamed into place ([`super::placement::place`]): a new file, never a
//! write into the old one. Then `install` with the new copy writes the unit and starts it. When
//! the copy or the start fails, the copy of before goes back by a rename and its unit starts
//! again; the next start of the app tries again. No other process is signalled: the holder of
//! the agent threads (THR-113) and its programs stay, and the update does not wait for them. A
//! copy that is not installed is never installed here: that is the install of HMD-053.
//!
//! Each run (`--version`, `stop`, `install`) has a deadline; a run past it is killed with every
//! process it started and counts as a failure, as above (a `stop` past it may have stopped the
//! unit, so the unit of the copy starts again). Each failed step writes one line with its name
//! to the log of the app, and a line goes there before the first step, so that an update that
//! the end of the app cuts leaves a trace.
//!
//! Each build replaces an older copy, also a build with an ad hoc signature, which the Keychain
//! sees as a new program and asks the person about once (R-NEW-updateall; it was R-321 option A).
//! What guards the copy then: the carried daemon is only the one beside the running app, whose
//! seal must verify ([`Swap::sealed`]); the folder `~/.local/bin` and each folder above it pass
//! the checks of the install, and the copy and the copy of before are plain files of this user,
//! never links; the new copy is checked under its fresh name before its rename (its seal and its
//! version), so a change of the bundle after the check puts no other program in place; and the
//! new daemon must answer with its version after its start ([`Swap::healthy`]), or the copy of
//! before runs again. After a good start the copy of before stays as `anima-daemon.<version>`,
//! the last one only, so the person can go back by hand.

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

/// `--version` prints one line and touches nothing; a program that takes longer does not answer.
const VERSION_DEADLINE: Duration = Duration::from_secs(10);

/// What the update needs; each path is given, so that a test runs it in a folder of its own.
pub struct Swap<'a> {
    /// The installed copy: `~/.local/bin/anima-daemon` of the user.
    pub copy: &'a Path,
    /// The daemon that the app carries.
    pub carried: &'a Path,
    pub user: User,
    pub env: &'a [(&'static str, OsString)],
    /// The deadline of the daemon's `stop` and of its `install`, which starts the unit again.
    pub restart: Duration,
    pub stop: &'a AtomicBool,
    /// Whether the seal of the bundle of the running app verifies: no update from a bundle whose
    /// files changed after it was signed.
    pub sealed: bool,
    /// The check of the seal of the new copy under its fresh name, before its rename.
    pub signed: &'a dyn Fn(&Path) -> bool,
    /// Whether the daemon that the unit started answers with this version within a short time.
    pub healthy: &'a dyn Fn(Version) -> bool,
}

/// What the update did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Updated {
    /// No copy is installed, so nothing is updated.
    NoCopy,
    /// The version of the carried daemon could not be read, so it cannot be shown to be newer.
    CarriedUnknown,
    /// The copy has the version of the carried daemon.
    Unchanged,
    /// The copy is newer than the carried daemon, and stays.
    KeptNewer,
    /// The copy was replaced, and its unit runs the new one.
    Replaced,
    /// The carried daemon is newer, but the seal of the bundle of the app does not verify: the
    /// copy stays.
    NotSealed,
}

/// Where the copy of before waits for the unit to start the new copy: `<copy>.previous`.
pub fn previous(copy: &Path) -> PathBuf {
    let mut name = copy.as_os_str().to_owned();
    name.push(".previous");
    PathBuf::from(name)
}

/// Compares the carried daemon with the installed copy and replaces the copy when the carried
/// one is newer (or the copy's version cannot be read, [`version::decide`]).
///
/// # Errors
/// [`DaemonError::Untrusted`] when a folder, the copy or the copy of before fails the checks of
/// the install (nothing was moved); [`DaemonError::Copy`], [`DaemonError::NotBundled`] or
/// [`DaemonError::Unchecked`] when the new copy was not put in place; [`DaemonError::Install`],
/// [`DaemonError::TimedOut`], [`DaemonError::Failed`] or [`DaemonError::Unhealthy`] when its
/// unit did not start or its daemon did not answer; [`DaemonError::Remove`] when the copy of
/// before could not be kept after the start. In each case but the first and the last, the copy
/// of before is back in place.
pub fn update_here(swap: &Swap<'_>) -> Result<Updated, DaemonError> {
    let aside = previous(swap.copy);
    if fs::symlink_metadata(swap.copy).is_err() && placement::file_trusted(&aside, swap.user) {
        // An update stopped between the two renames: the copy of before goes back.
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

/// The version of `program` by its `--version`.
fn read(swap: &Swap<'_>, program: &Path) -> Option<Version> {
    version::read(program, swap.env, VERSION_DEADLINE, swap.stop)
}

/// The checks of the hand install, before anything is moved or run: the folder of the copy and
/// each folder above it, then the copy and a copy of before that waits, each a plain file of
/// this user that no other user can change. A link at either place is refused, never followed.
fn trusted(swap: &Swap<'_>, aside: &Path) -> Result<(), DaemonError> {
    let folder = swap.copy.parent().ok_or(DaemonError::Untrusted)?;
    placement::trusted_folder(folder, swap.user).map_err(DaemonError::of_placement)?;
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

/// Stops the unit, moves the copy aside (unless an earlier copy already waits there), puts the
/// carried daemon in its place and starts the unit again. When the copy is not put in place, the
/// copy of before goes back and its unit starts again.
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
        // The copy of before starts again; the error that matters is the first one.
        let _ = restart(swap, &real(swap.copy), BEFORE);
        Err(error)
    };
    if fs::symlink_metadata(aside).is_err() && fs::rename(swap.copy, aside).is_err() {
        return failed(DaemonError::Copy);
    }
    // The new copy is checked under its fresh name: the bytes that will run, not the source.
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

/// `stop` with the carried daemon: the unit stops, and its file and the token stay. No file is
/// changed yet when it fails. A stop that ran past its deadline is killed, and may have stopped
/// the unit, so the unit of the copy starts again.
fn stop(swap: &Swap<'_>) -> Result<(), DaemonError> {
    let Err(error) = stop_with(swap, swap.carried) else {
        return Ok(());
    };
    let failed = failed_step("stop", error, DaemonError::Failed);
    if error == RunError::TimedOut {
        // The error that the person needs is the one of the stop.
        let _ = restart(swap, &real(swap.copy), BEFORE);
    }
    Err(failed)
}

/// `stop` with `program`.
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

/// The name of the `install` of the copy of before, in the log.
const BEFORE: &str = "install of the copy of before";
/// The name of the `install` of the new copy, in the log.
const NEW: &str = "install of the new copy";

/// The error of the step `name` that did not succeed, after one line in the log of the app with
/// the name and the code of the error (never a path or an OS text).
fn failed_step(name: &str, error: RunError, failed: DaemonError) -> DaemonError {
    let error = DaemonError::of_run(error, failed);
    log::warn!(
        "the daemon update: the step `{name}` failed: {}",
        error.code()
    );
    error
}

/// The real path of `copy`, the path that the install runs; `copy` itself when it has none.
fn real(copy: &Path) -> PathBuf {
    fs::canonicalize(copy).unwrap_or_else(|_| copy.to_owned())
}

/// An update that a stop of the app cut after the new copy was put in place: its unit starts
/// again, so that it runs the copy that is there.
fn finish_earlier(swap: &Swap<'_>, aside: &Path, carried: Version) -> Result<(), DaemonError> {
    if aside.is_file() {
        log::info!("the daemon update: an earlier update was cut; its unit starts again");
        let before = read(swap, aside);
        let running = read(swap, swap.copy).unwrap_or(carried);
        return started(swap, &real(swap.copy), aside, (running, before));
    }
    Ok(())
}

/// Starts the unit with `installed` and waits for the new daemon to answer with its version,
/// then keeps the copy of before as `anima-daemon.<its version>`. When the start fails or the
/// daemon does not answer, the copy of before goes back and its unit starts again.
/// `versions` are the version of the new copy and of the copy of before.
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
        // The new daemon stops before the copy of before goes back.
        let _ = stop_with(swap, installed);
        Err(DaemonError::Unhealthy)
    });
    if let Err(error) = begun {
        if put_back(swap.copy, aside).is_ok() {
            log::warn!("the daemon update: the copy of before is back; its unit starts again");
            // The error that the person needs is the one of the new copy; the next start of
            // the app tries the update again.
            let _ = restart(swap, installed, BEFORE);
        } else {
            log::warn!("the daemon update: the copy of before could not be put back");
        }
        return Err(error);
    }
    kept::keep(swap.copy, aside, versions.1)
}

/// `install` with `program`: it writes the unit again and starts it, so the unit runs the copy
/// that is in place now. The token stays where it is. `name` names the step in the log.
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

/// Renames the copy of before over the copy, and flushes the folder.
fn put_back(copy: &Path, aside: &Path) -> io::Result<()> {
    fs::rename(aside, copy)?;
    File::open(copy.parent().unwrap_or_else(|| Path::new("/")))?.sync_all()
}

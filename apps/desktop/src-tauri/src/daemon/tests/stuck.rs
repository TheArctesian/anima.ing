//! The update of the installed copy (HMD-055) when a step does not end, and on a build whose
//! daemon would be a new program to the Keychain (R-NEW-adhocswap). Stand-in daemons as in
//! [`super::updates`]; no real unit, Keychain or `~/.local/bin` is touched.

use std::{fs, path::Path, process::Command, time::Duration};

use super::updates::{NEW, NEW_COPY, OLD, OLD_COPY, bin, copy_text, installed, update_with};
use crate::daemon::{DaemonError, signature::Identity, update::Updated};

/// The deadline of each `stop` and `install`, for a stand-in that never ends.
const SHORT: Duration = Duration::from_secs(1);

/// `then` of a stand-in whose `verb` writes its process number to `pid` and never ends.
fn hangs_at(verb: &str, pid: &Path) -> String {
    format!(
        "[ \"$1\" = {verb} ] && {{ echo $$ > '{}'; sleep 600; }}; exit 0",
        pid.display()
    )
}

/// Whether the process whose number is in `pid` still runs.
fn still_runs(pid: &Path) -> bool {
    let pid = fs::read_to_string(pid).expect("the process number");
    Command::new("/bin/kill")
        .args(["-0", pid.trim()])
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn hmd_055_an_install_that_does_not_end_is_killed_and_the_copy_of_before_starts_again() {
    let folder = tempfile::tempdir().expect("a folder");
    let pid = folder.path().join("pid");
    let (rig, old, _) = installed(OLD, NEW, &hangs_at("install", &pid));

    assert_eq!(
        update_with(&rig, SHORT, Identity::Kept),
        Err(DaemonError::TimedOut)
    );

    assert!(!still_runs(&pid), "the stuck install was killed");
    assert_eq!(copy_text(&rig), old);
    assert_eq!(bin(&rig), ["anima-daemon"]);
    let runs = rig.runs();
    assert!(
        runs.starts_with(&format!(
            "new stop {OLD_COPY} prev:\nnew install {NEW_COPY} prev:"
        )),
        "{runs}"
    );
    assert!(
        runs.ends_with(&format!("\nold install {OLD_COPY} prev:\n")),
        "the unit of the copy of before starts again: {runs}"
    );
}

#[test]
fn hmd_055_a_stop_that_does_not_end_is_killed_and_the_unit_of_the_copy_starts_again() {
    let folder = tempfile::tempdir().expect("a folder");
    let pid = folder.path().join("pid");
    let (rig, old, _) = installed(OLD, NEW, &hangs_at("stop", &pid));

    assert_eq!(
        update_with(&rig, SHORT, Identity::Kept),
        Err(DaemonError::TimedOut)
    );

    assert!(!still_runs(&pid), "the stuck stop was killed");
    assert_eq!(copy_text(&rig), old);
    assert_eq!(
        rig.runs(),
        format!("new stop {OLD_COPY} prev:\nold install {OLD_COPY} prev:\n")
    );
}

#[test]
fn hmd_055_a_carried_daemon_that_is_a_new_program_to_the_keychain_does_not_replace_the_copy() {
    let (rig, old, _) = installed(OLD, NEW, "exit 0");

    assert_eq!(
        update_with(&rig, SHORT, Identity::NewEachBuild),
        Ok(Updated::KeptForKeychain)
    );

    assert_eq!(copy_text(&rig), old);
    assert_eq!(rig.runs(), "", "no stop and no install");
}

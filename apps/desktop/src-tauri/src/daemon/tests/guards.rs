//! The update guards of R-437.

use std::{fs, os::unix::fs::PermissionsExt, path::Path};

use super::updates::{
    Checks, NEW, NEW_COPY, OLD, OLD_COPY, bin, copy_text, installed, update_checked,
};
use crate::daemon::{
    DaemonError,
    update::{Updated, copy_trusted, previous},
};

fn checked(rig: &super::rig::Rig, checks: Checks) -> Result<Updated, DaemonError> {
    update_checked(rig, &checks)
}

#[test]
fn r_updateall_a_link_at_the_copy_or_at_the_copy_of_before_is_refused_and_not_followed() {
    let (rig, _, _) = installed(OLD, NEW, "exit 0");
    assert!(
        copy_trusted(&rig.installed(), rig.user),
        "a plain copy of this user"
    );
    let elsewhere = rig.dir.path().join("elsewhere");
    fs::rename(rig.installed(), &elsewhere).expect("moved");
    std::os::unix::fs::symlink(&elsewhere, rig.installed()).expect("a link at the copy");
    assert!(
        !copy_trusted(&rig.installed(), rig.user),
        "the start of the app does not run a copy behind a link to read its version"
    );

    assert_eq!(
        checked(&rig, Checks::default()),
        Err(DaemonError::Untrusted)
    );
    assert_eq!(rig.runs(), "", "nothing ran");
    assert!(
        fs::symlink_metadata(rig.installed())
            .expect("there")
            .is_symlink()
    );

    let (rig, old, _) = installed(OLD, NEW, "exit 0");
    std::os::unix::fs::symlink(&elsewhere, previous(&rig.installed())).expect("a link aside");
    assert_eq!(
        checked(&rig, Checks::default()),
        Err(DaemonError::Untrusted)
    );
    assert_eq!(copy_text(&rig), old);
    assert_eq!(rig.runs(), "");
}

#[test]
fn r_updateall_a_copy_that_others_may_change_is_refused() {
    let (rig, old, _) = installed(OLD, NEW, "exit 0");
    fs::set_permissions(rig.installed(), fs::Permissions::from_mode(0o757)).expect("chmod");
    assert!(
        !copy_trusted(&rig.installed(), rig.user),
        "nor a copy that others may change"
    );

    assert_eq!(
        checked(&rig, Checks::default()),
        Err(DaemonError::Untrusted)
    );

    assert_eq!(copy_text(&rig), old);
    assert_eq!(rig.runs(), "");
}

#[test]
fn r_updateall_a_bundle_whose_seal_does_not_verify_updates_nothing() {
    let (rig, old, _) = installed(OLD, NEW, "exit 0");
    let checks = Checks {
        sealed: false,
        ..Checks::default()
    };

    assert_eq!(checked(&rig, checks), Ok(Updated::NotSealed));

    assert_eq!(copy_text(&rig), old);
    assert_eq!(rig.runs(), "");
}

#[test]
fn r_updateall_a_new_copy_that_fails_its_check_is_never_put_in_place() {
    let (rig, old, _) = installed(OLD, NEW, "exit 0");
    let checks = Checks {
        signed: |part: &Path| !part.to_string_lossy().ends_with(".part"),
        ..Checks::default()
    };

    assert_eq!(checked(&rig, checks), Err(DaemonError::Unchecked));

    assert_eq!(copy_text(&rig), old);
    assert_eq!(bin(&rig), ["anima-daemon"], "no part file stays");
    assert_eq!(
        rig.runs(),
        format!("new stop {OLD_COPY} prev:\nold install {OLD_COPY} prev:\n"),
        "the copy of before starts again"
    );
}

#[test]
fn r_updateall_a_new_daemon_that_does_not_answer_gives_way_to_the_copy_of_before() {
    let (rig, old, _) = installed(OLD, NEW, "exit 0");
    let checks = Checks {
        healthy: |_| false,
        ..Checks::default()
    };

    assert_eq!(checked(&rig, checks), Err(DaemonError::Unhealthy));

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
        runs.contains(&format!("\nnew stop {NEW_COPY} prev:")),
        "the new daemon stops: {runs}"
    );
    assert!(
        runs.ends_with(&format!("old install {OLD_COPY} prev:\n")),
        "then the copy of before starts again: {runs}"
    );
}

#[test]
fn r_updateall_only_the_last_copy_of_before_stays() {
    let (rig, _, _) = installed(OLD, NEW, "exit 0");
    let earlier = rig.home.join(".local/bin/anima-daemon.0.1.8");
    fs::write(&earlier, "an earlier copy").expect("an earlier copy");
    let not_a_version = rig.home.join(".local/bin/anima-daemon.notes");
    fs::write(&not_a_version, "words").expect("a file of the person");

    assert_eq!(checked(&rig, Checks::default()), Ok(Updated::Replaced));

    assert_eq!(
        bin(&rig),
        ["anima-daemon", "anima-daemon.0.1.9", "anima-daemon.notes"]
    );
}

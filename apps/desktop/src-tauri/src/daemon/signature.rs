//! Whether the daemon that the app carries keeps its identity in the macOS Keychain from one
//! build to the next (R-321). The Keychain lets a program read an item when the
//! program satisfies the designated requirement that the item recorded. A Developer ID build
//! names its team, so each later build of the team satisfies it; an ad hoc build names the
//! hash of its own code, so each build is a new program, and its first read of the token waits
//! for the person to allow it: after an update to such a build, the Keychain asks the person once
//! (R-NEW-updateall). Linux has no Keychain: there it always keeps its identity. The seal of
//! the bundle and of the new copy is checked here too ([`sealed`], [`sealed_by`]).

use std::path::Path;

/// What the Keychain sees in a new build of the carried daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identity {
    /// A signature with a team (Developer ID), or a system with no Keychain.
    Kept,
    /// An ad hoc signature, no signature, or one that could not be read.
    NewEachBuild,
}

/// The identity from what `codesign -d --verbose=2` showed of the program (on standard error)
/// and whether it succeeded. Only a `TeamIdentifier=` of ten capital letters and digits keeps
/// the identity; anything else, `not set` (ad hoc) included, does not.
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "read only on macOS and by its tests")
)]
pub fn identity(success: bool, shown: &str) -> Identity {
    if team(success, shown).is_some() {
        Identity::Kept
    } else {
        Identity::NewEachBuild
    }
}

/// The team that signed, from what `codesign -d --verbose=2` showed and whether it succeeded:
/// a `TeamIdentifier=` of ten capital letters and digits, else none.
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "read only on macOS and by its tests")
)]
pub fn team(success: bool, shown: &str) -> Option<String> {
    let team = shown
        .lines()
        .find_map(|line| line.strip_prefix("TeamIdentifier="))
        .map(str::trim)?;
    let is_team = team.len() == 10
        && team
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit());
    (success && is_team).then(|| team.to_owned())
}

/// The identity of `program` on this system: on macOS from `/usr/bin/codesign`, which must
/// answer in 10 s; a run that does not is [`Identity::NewEachBuild`].
#[cfg(target_os = "macos")]
pub fn of_program(program: &Path) -> Identity {
    codesign(&["-d", "--verbose=2"], program).map_or(Identity::NewEachBuild, |(success, shown)| {
        identity(success, &shown)
    })
}

/// Whether the seal of `path` (an app bundle, or a program) verifies: its code and its
/// resources did not change since it was signed. An ad hoc seal says nothing of who signed it.
#[cfg(target_os = "macos")]
pub fn sealed(path: &Path) -> bool {
    codesign(&["--verify", "--strict"], path).is_some_and(|(success, _)| success)
}

/// No seals outside macOS: the package manager of the system checks the files of the app.
#[cfg(not(target_os = "macos"))]
pub const fn sealed(_path: &Path) -> bool {
    true
}

/// The team that signed `path` (an app bundle); none for an ad hoc signature or one that could
/// not be read.
#[cfg(target_os = "macos")]
pub fn team_of(path: &Path) -> Option<String> {
    codesign(&["-d", "--verbose=2"], path).and_then(|(success, shown)| team(success, &shown))
}

/// No teams outside macOS.
#[cfg(not(target_os = "macos"))]
pub const fn team_of(_path: &Path) -> Option<String> {
    None
}

/// Whether the seal of the program `path` verifies and, when `team` names one, the program was
/// signed by that team with a Developer ID: a program of a different signer, also one with a
/// seal of its own, does not pass. With no team (an ad hoc build) only the seal is checked.
#[cfg(target_os = "macos")]
pub fn sealed_by(path: &Path, team: Option<&str>) -> bool {
    let Some(team) = team else {
        return sealed(path);
    };
    let requirement =
        format!("=anchor apple generic and certificate leaf[subject.OU] = \"{team}\"");
    codesign(&["--verify", "--strict", "-R", &requirement], path)
        .is_some_and(|(success, _)| success)
}

/// No seals outside macOS.
#[cfg(not(target_os = "macos"))]
pub const fn sealed_by(_path: &Path, _team: Option<&str>) -> bool {
    true
}

/// Runs `/usr/bin/codesign` with `args` on `path`, with no environment: whether it succeeded and
/// what it showed on standard error. None when it did not run or did not end in 10 s.
#[cfg(target_os = "macos")]
fn codesign(args: &[&str], path: &Path) -> Option<(bool, String)> {
    use std::{
        io::Read as _,
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };
    let mut child = Command::new("/usr/bin/codesign")
        .args(args)
        .arg(path)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let end = Instant::now() + Duration::from_secs(10);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < end => thread::sleep(Duration::from_millis(20)),
            // Past the deadline, or the wait failed: no answer, and a failed kill of a program
            // that is ending adds nothing to it.
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let mut shown = String::new();
    child.stderr.take()?.read_to_string(&mut shown).ok()?;
    Some((status.success(), shown))
}

/// No Keychain outside macOS: each build keeps its identity.
#[cfg(not(target_os = "macos"))]
pub const fn of_program(_program: &Path) -> Identity {
    Identity::Kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmd_055_only_a_team_keeps_the_identity_of_the_daemon_in_the_keychain() {
        let developer_id = "Identifier=anima-daemon\nAuthority=Developer ID Application: Anima (ABCDE12345)\nTeamIdentifier=ABCDE12345\n";
        let ad_hoc = "Identifier=anima-daemon\nSignature=adhoc\nTeamIdentifier=not set\n";
        let unsigned = "anima-daemon: code object is not signed at all\n";
        assert_eq!(identity(true, developer_id), Identity::Kept);
        assert_eq!(identity(true, ad_hoc), Identity::NewEachBuild);
        assert_eq!(identity(false, unsigned), Identity::NewEachBuild);
        assert_eq!(
            identity(false, developer_id),
            Identity::NewEachBuild,
            "a failed read keeps nothing"
        );
        assert_eq!(
            identity(true, "TeamIdentifier=abcde12345\n"),
            Identity::NewEachBuild
        );
        assert_eq!(identity(true, ""), Identity::NewEachBuild);
        assert_eq!(team(true, developer_id).as_deref(), Some("ABCDE12345"));
        assert_eq!(team(true, ad_hoc), None);
    }
}

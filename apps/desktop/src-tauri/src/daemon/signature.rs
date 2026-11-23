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

/// Who signed the app that runs, as the code in memory shows it: a program put in the bundle
/// after the start does not change it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signer {
    /// A team with a Developer ID: each part must be signed by it.
    Team(String),
    /// An ad hoc signature (a build of a person): each part needs only its seal.
    AdHoc,
    /// The signature could not be read or is neither: no part passes.
    Unknown,
}

/// The signer from what `codesign -d --verbose=3` showed of the running app and whether it
/// succeeded. It is taken only once the running code verifies against [`pinned`].
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "read only on macOS and by its tests")
)]
pub fn signer(success: bool, shown: &str) -> Signer {
    if !success {
        return Signer::Unknown;
    }
    if let Some(team) = team(success, shown) {
        return Signer::Team(team);
    }
    let ad_hoc = shown.lines().any(|line| line.trim() == "Signature=adhoc");
    if ad_hoc {
        Signer::AdHoc
    } else {
        Signer::Unknown
    }
}

/// The requirement that the running code must meet for `signer` to stand: the Developer ID of
/// its team, or for an ad hoc app the hash of the code that was read (`CDHash=`, 40 hex
/// digits). One verify against it closes the gap between the read and the check.
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "read only on macOS and by its tests")
)]
pub fn pinned(signer: &Signer, shown: &str) -> Option<String> {
    match signer {
        Signer::Team(team) => Some(developer_id(team)),
        Signer::AdHoc => shown
            .lines()
            .find_map(|line| line.strip_prefix("CDHash="))
            .map(str::trim)
            .filter(|hash| hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .map(|hash| format!("=cdhash H\"{hash}\"")),
        Signer::Unknown => None,
    }
}

/// The signer of this running app, read from its process and then verified against what was
/// read in one run of codesign: the team is never taken from the files of the bundle, which can
/// change after the start.
#[cfg(target_os = "macos")]
pub fn of_this_app() -> Signer {
    let pid = std::process::id().to_string();
    let process = Path::new(&pid);
    let Some((success, shown)) = codesign(&["-d", "--verbose=3"], process) else {
        return Signer::Unknown;
    };
    let read = signer(success, &shown);
    let Some(requirement) = pinned(&read, &shown) else {
        return Signer::Unknown;
    };
    let verified =
        codesign(&["--verify", "-R", &requirement], process).is_some_and(|(success, _)| success);
    if verified { read } else { Signer::Unknown }
}

/// No seals outside macOS.
#[cfg(not(target_os = "macos"))]
pub const fn of_this_app() -> Signer {
    Signer::AdHoc
}

/// Whether the seal of `path` (the bundle or a program) verifies for `signer`: for a team, also
/// that the team signed it with a Developer ID, so a program of a different signer, also one
/// with a seal of its own, does not pass; for an ad hoc app only the seal; for an unknown
/// signer nothing passes.
#[cfg(target_os = "macos")]
pub fn sealed_by(path: &Path, signer: &Signer) -> bool {
    match signer {
        Signer::Team(team) => {
            let requirement = developer_id(team);
            codesign(&["--verify", "--strict", "-R", &requirement], path)
                .is_some_and(|(success, _)| success)
        }
        Signer::AdHoc => sealed(path),
        Signer::Unknown => false,
    }
}

/// No seals outside macOS.
#[cfg(not(target_os = "macos"))]
pub const fn sealed_by(_path: &Path, _signer: &Signer) -> bool {
    true
}

/// The requirement of a Developer ID Application certificate of `team`: Apple's chain, the
/// Developer ID intermediate, the Developer ID Application leaf and the team.
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "read only on macOS and by its tests")
)]
fn developer_id(team: &str) -> String {
    format!(
        "=anchor apple generic and certificate 1[field.1.2.840.113635.100.6.2.6] exists and certificate leaf[field.1.2.840.113635.100.6.1.13] exists and certificate leaf[subject.OU] = \"{team}\""
    )
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

    #[test]
    fn hmd_055_only_a_team_or_an_ad_hoc_app_pinned_to_what_was_read_lets_a_part_pass() {
        let by_team = "Identifier=ing.anima\nTeamIdentifier=ABCDE12345\n";
        let hash = "e8ee3a3354352b7eb3bb2766cfe34b880f8b4317";
        let ad_hoc = format!(
            "Identifier=ing.anima\nCDHash={hash}\nSignature=adhoc\nTeamIdentifier=not set\n"
        );
        let team = signer(true, by_team);
        assert_eq!(team, Signer::Team("ABCDE12345".to_owned()));
        assert_eq!(pinned(&team, by_team), Some(developer_id("ABCDE12345")));
        assert_eq!(signer(true, &ad_hoc), Signer::AdHoc);
        assert_eq!(
            pinned(&Signer::AdHoc, &ad_hoc),
            Some(format!("=cdhash H\"{hash}\"")),
            "an ad hoc app is held to the code that was read"
        );
        assert_eq!(
            pinned(&Signer::AdHoc, "Signature=adhoc\nCDHash=12\"34\n"),
            None,
            "a hash that is not 40 hex digits pins nothing, so nothing passes"
        );
        assert_eq!(
            signer(false, by_team),
            Signer::Unknown,
            "a read that failed falls closed, never to the seal alone"
        );
        assert_eq!(signer(true, "Identifier=ing.anima\n"), Signer::Unknown);
        assert_eq!(pinned(&Signer::Unknown, by_team), None);
        assert!(developer_id_requirement_names_the_team());
    }

    fn developer_id_requirement_names_the_team() -> bool {
        let requirement = developer_id("ABCDE12345");
        requirement.contains("1.2.840.113635.100.6.2.6")
            && requirement.contains("1.2.840.113635.100.6.1.13")
            && requirement.ends_with("certificate leaf[subject.OU] = \"ABCDE12345\"")
    }
}

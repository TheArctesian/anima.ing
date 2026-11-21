//! Whether the daemon that the app carries keeps its identity in the macOS Keychain from one
//! build to the next (R-NEW-adhocswap). The Keychain lets a program read an item when the
//! program satisfies the designated requirement that the item recorded. A Developer ID build
//! names its team, so each later build of the team satisfies it; an ad hoc build names the
//! hash of its own code, so each build is a new program, and its first read of the token waits
//! for the person to allow it. The update at the start of the app then swaps the daemon only
//! when the carried daemon has a team. Linux has no Keychain: there it always keeps its identity.

use std::path::Path;

/// What the Keychain sees in a new build of the carried daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identity {
    /// A signature with a team (Developer ID), or a system with no Keychain.
    Kept,
    /// An ad hoc signature, no signature, or one that could not be read.
    #[cfg_attr(
        all(not(target_os = "macos"), not(test)),
        expect(dead_code, reason = "made only on macOS and by the tests")
    )]
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
    let team = shown
        .lines()
        .find_map(|line| line.strip_prefix("TeamIdentifier="))
        .map(str::trim);
    let is_team = |team: &str| {
        team.len() == 10
            && team
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    };
    if success && team.is_some_and(is_team) {
        Identity::Kept
    } else {
        Identity::NewEachBuild
    }
}

/// The identity of `program` on this system: on macOS from `/usr/bin/codesign`, which must
/// answer in 10 s; a run that does not is [`Identity::NewEachBuild`], so nothing is swapped.
#[cfg(target_os = "macos")]
pub fn of_program(program: &Path) -> Identity {
    use std::{
        io::Read as _,
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };
    let started = Command::new("/usr/bin/codesign")
        .args(["-d", "--verbose=2"])
        .arg(program)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn();
    let Ok(mut child) = started else {
        return Identity::NewEachBuild;
    };
    let end = Instant::now() + Duration::from_secs(10);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < end => thread::sleep(Duration::from_millis(20)),
            // Past the deadline, or the wait failed: the answer is the safe one, and a failed
            // kill of a program that is ending adds nothing to it.
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Identity::NewEachBuild;
            }
        }
    };
    let mut shown = String::new();
    if let Some(mut errors) = child.stderr.take()
        && errors.read_to_string(&mut shown).is_err()
    {
        return Identity::NewEachBuild;
    }
    identity(status.success(), &shown)
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
    }
}

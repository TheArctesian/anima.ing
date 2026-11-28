//! Code signatures on macOS through `codesign`; each `-d` answer is checked again by `--verify`, as the file of a running process can be replaced.

use super::{PeerError, Program};
#[cfg(target_os = "macos")]
use std::{
    io::{self, Read as _},
    process::Stdio,
    time::{Duration, Instant},
};

/// By its full path, as the daemon runs it with an empty environment.
#[cfg(target_os = "macos")]
const CODESIGN: &str = "/usr/bin/codesign";

#[cfg(target_os = "macos")]
const SIGNATURE_LIMIT: Duration = Duration::from_secs(5);

/// The link program's signing identifier, alone or with the `-<hex>` suffix that a signature with no bundle can add.
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "read only on macOS and by its tests")
)]
const LINK: &str = "anima-link";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamId(String);

impl TeamId {
    /// Only the form of a Team ID, so that no other text goes into a code requirement.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let valid = text.len() == 10
            && text
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit());
        valid.then(|| Self(text.to_owned()))
    }

    /// The same Developer ID requirement that the builder of a release verifies (BLD-083).
    #[must_use]
    pub fn requirement(&self) -> String {
        format!(
            "anchor apple generic and certificate 1[field.1.2.840.113635.100.6.2.6] exists and \
             certificate leaf[field.1.2.840.113635.100.6.1.13] exists and \
             certificate leaf[subject.OU] = \"{}\"",
            self.0
        )
    }
}

/// Accepted only when `codesign --verify` of the running daemon agrees, so a replaced file is an error, not an unsigned daemon.
#[cfg(target_os = "macos")]
pub(super) fn own_program() -> Result<Program, PeerError> {
    let me = std::process::id().to_string();
    let (shown, text) = codesign_now(&["-d".into(), "--verbose=2".into(), me.clone()])?;
    let program = shown_program(shown, &text).ok_or(PeerError::OwnSignature)?;
    let requirement = format!("-R={}", own_requirement(&program));
    let (verified, _) = codesign_now(&["--verify".into(), requirement, me])?;
    if verified {
        Ok(program)
    } else {
        Err(PeerError::OwnSignature)
    }
}

#[cfg(target_os = "macos")]
pub(super) async fn is_the_link(process: i32, team: &TeamId) -> Result<bool, PeerError> {
    if process <= 0 {
        return Ok(false);
    }
    let pid = process.to_string();
    let Some((shown, text)) = codesign(&["-d", "--verbose=2", &pid]).await? else {
        return Ok(false);
    };
    let Some(identifier) = shown.then(|| link_identifier(&text)).flatten() else {
        return Ok(false);
    };
    let requirement = format!("-R={}", link_requirement(identifier, team));
    Ok(codesign(&["--verify", &requirement, &pid])
        .await?
        .is_some_and(|(verified, _)| verified))
}

/// No signature is read outside macOS, so [`Program::SignedBy`] admits nothing there.
#[cfg(not(target_os = "macos"))]
#[expect(clippy::unused_async, reason = "the same signature as the macOS check")]
pub(super) async fn is_the_link(_process: i32, _team: &TeamId) -> Result<bool, PeerError> {
    Ok(false)
}

#[cfg(target_os = "macos")]
async fn codesign(args: &[&str]) -> io::Result<Option<(bool, String)>> {
    let mut command = tokio::process::Command::new(CODESIGN);
    command
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    match tokio::time::timeout(SIGNATURE_LIMIT, command.output()).await {
        Ok(output) => {
            let output = output?;
            let text = String::from_utf8_lossy(&output.stderr).into_owned();
            Ok(Some((output.status.success(), text)))
        }
        Err(_) => Ok(None),
    }
}

#[cfg(target_os = "macos")]
fn codesign_now(args: &[String]) -> io::Result<(bool, String)> {
    let mut child = std::process::Command::new(CODESIGN)
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let end = Instant::now() + SIGNATURE_LIMIT;
    loop {
        if let Some(status) = child.try_wait()? {
            let mut text = String::new();
            if let Some(mut errors) = child.stderr.take() {
                errors.read_to_string(&mut text)?;
            }
            return Ok((status.success(), text));
        }
        if Instant::now() >= end {
            // The time limit is the caller's error; a failed kill or wait of an ending program adds nothing.
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::ErrorKind::TimedOut.into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// `codesign -d` prints on standard error; no Developer ID signature (none, ad hoc, no team) is [`Program::OfTheUser`].
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "read only on macOS and by its tests")
)]
fn shown_program(success: bool, shown: &str) -> Option<Program> {
    if !success {
        return shown
            .contains("code object is not signed at all")
            .then_some(Program::OfTheUser);
    }
    let team = shown
        .lines()
        .find_map(|line| line.strip_prefix("TeamIdentifier="))?
        .trim();
    if team == "not set" {
        return Some(Program::OfTheUser);
    }
    TeamId::parse(team).map(Program::SignedBy)
}

/// With no Developer ID, no Apple anchor at all, which fails for a signed daemon whose file shows none.
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "read only on macOS and by its tests")
)]
fn own_requirement(program: &Program) -> String {
    match program {
        Program::SignedBy(team) => team.requirement(),
        Program::OfTheUser => "! anchor apple generic".to_owned(),
    }
}

#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "read only on macOS and by its tests")
)]
fn link_identifier(shown: &str) -> Option<&str> {
    named_identifier(shown, LINK)
}

#[cfg(target_os = "macos")]
pub(crate) fn daemon_requirement(process: u32) -> Option<String> {
    let (shown, text) =
        codesign_now(&["-d".into(), "--verbose=4".into(), process.to_string()]).ok()?;
    let identifier = named_identifier(&text, "anima-daemon")?;
    let hash = text.lines().find_map(|line| line.strip_prefix("CDHash="))?;
    let hardened = text
        .lines()
        .filter(|line| line.starts_with("CodeDirectory "))
        .any(|line| {
            line.split_once("flags=")
                .and_then(|(_, rest)| rest.split_once('('))
                .and_then(|(_, rest)| rest.split_once(')'))
                .is_some_and(|(flags, _)| flags.split(',').any(|flag| flag == "runtime"))
        });
    if !shown || !hardened || hash.len() != 40 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    let team = TeamId::parse("7GDLG48PV6")?;
    Some(format!(
        r#"{} and cdhash H"{hash}" and !entitlement["com.apple.security.get-task-allow"] and !entitlement["com.apple.security.cs.disable-library-validation"] and !entitlement["com.apple.security.cs.allow-dyld-environment-variables"]"#,
        link_requirement(identifier, &team)
    ))
}

fn named_identifier<'a>(shown: &'a str, name: &str) -> Option<&'a str> {
    let identifier = shown
        .lines()
        .find_map(|line| line.strip_prefix("Identifier="))?
        .trim();
    let suffix_ok = |rest: &str| {
        rest.strip_prefix('-')
            .is_some_and(|hex| !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()))
    };
    let named = identifier
        .strip_prefix(name)
        .is_some_and(|rest| rest.is_empty() || suffix_ok(rest));
    named.then_some(identifier)
}

#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "read only on macOS and by its tests")
)]
fn link_requirement(identifier: &str, team: &TeamId) -> String {
    format!("identifier \"{identifier}\" and {}", team.requirement())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn team() -> TeamId {
        TeamId::parse("ABCDE12345").expect("a team")
    }

    #[test]
    fn only_the_form_of_a_team_id_goes_into_a_requirement() {
        assert!(TeamId::parse("ABCDE12345").is_some());
        for bad in ["abcde12345", "ABCDE1234", "ABCDE123456", "ABCDE\"1234", ""] {
            assert_eq!(TeamId::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn what_the_daemon_shows_of_itself_names_the_program_to_expect_or_nothing() {
        let signed = "Executable=/x\nIdentifier=anima-daemon\nTeamIdentifier=ABCDE12345\n";
        assert_eq!(shown_program(true, signed), Some(Program::SignedBy(team())));
        let ad_hoc = "Identifier=anima_daemon-55\nSignature=adhoc\nTeamIdentifier=not set\n";
        assert_eq!(shown_program(true, ad_hoc), Some(Program::OfTheUser));
        assert_eq!(
            shown_program(false, "1234: code object is not signed at all\n"),
            Some(Program::OfTheUser)
        );
        assert_eq!(shown_program(false, "codesign: no such process\n"), None);
        assert_eq!(shown_program(true, "Identifier=x\n"), None);
        assert_eq!(shown_program(true, "TeamIdentifier=bad\"team\n"), None);
    }

    #[test]
    fn a_daemon_that_shows_no_signature_must_run_with_no_apple_anchor() {
        assert_eq!(
            own_requirement(&Program::OfTheUser),
            "! anchor apple generic"
        );
        assert_eq!(
            own_requirement(&Program::SignedBy(team())),
            team().requirement()
        );
    }

    #[test]
    fn only_the_identifier_of_the_link_program_is_accepted_and_goes_into_the_requirement() {
        let shown =
            |id: &str| format!("Executable=/x\nIdentifier={id}\nTeamIdentifier=ABCDE12345\n");
        assert_eq!(link_identifier(&shown("anima-link")), Some("anima-link"));
        assert_eq!(
            link_identifier(&shown("anima-link-5555494489debf33")),
            Some("anima-link-5555494489debf33")
        );
        for other in [
            "anima-daemon",
            "anima-linker",
            "anima-link-",
            "anima-link-xyz",
            "anima-link\" or anchor apple",
        ] {
            assert_eq!(link_identifier(&shown(other)), None, "{other}");
        }
        assert_eq!(link_identifier("TeamIdentifier=ABCDE12345\n"), None);
        assert!(
            link_requirement("anima-link", &team()).starts_with("identifier \"anima-link\" and ")
        );
    }
}

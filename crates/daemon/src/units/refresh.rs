//! The refresh of the unit at the start of `run` (SF-259): an update replaces the program and not
//! the unit file, so a unit file that differs from the unit that this version renders is written
//! again. Only the unit of a Linux user is written here: `systemctl restart` is one job of the
//! service manager, which ends this process and starts the new unit. A `launchctl bootout` would
//! end this process before its `bootstrap`, and the system unit is in `/etc`, which the daemon
//! cannot write, so for those two the log says to run `install` again.

use std::io;
use std::path::{Path, PathBuf};

use super::install::{self, Host, InstallError};
use super::plan::{self, SYSTEM_HOME, SYSTEM_UNIT_FILE, Target};
use super::render::{self, Program, RenderError, Unit};
use super::step::{STEP_DEADLINE, run_step};
use crate::config::home::Home;
use crate::config::private_file;
use crate::readers::Platform;
use crate::report;

/// What the unit file on disk is, against the unit that this version renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Found {
    /// No unit file: the daemon was started by hand.
    Absent,
    Same,
    /// The file starts a different program, as the unit of an installed daemon does while a
    /// different build runs by hand. It is not changed.
    Other,
    /// The file starts this program and is not the unit that this version renders.
    Older,
}

/// # Errors
/// [`RenderError`] when the template does not hold the program.
pub(super) fn compare(
    unit: Unit,
    on_disk: Option<&str>,
    program: &Program<'_>,
) -> Result<Found, RenderError> {
    let Some(text) = on_disk else {
        return Ok(Found::Absent);
    };
    Ok(if text == render::render(unit, program)? {
        Found::Same
    } else if render::starts(unit, text, program) {
        Found::Older
    } else {
        Found::Other
    })
}

/// Refreshes the unit of this daemon, and says in the log what it did.
pub fn at_start(home: &Home) {
    match refresh(home) {
        Ok((Found::Older, file)) => report::unit_older(&file),
        Ok(_) => {}
        Err(error) => report::unit_refresh_failed(&error),
    }
}

/// `Found::Older` only for a unit that was not written again.
fn refresh(home: &Home) -> Result<(Found, PathBuf), InstallError> {
    let platform = Platform::current();
    let env = |name: &str| std::env::var(name).ok();
    let uid = private_file::running_user().map_err(InstallError::Binary)?;
    let program = std::env::current_exe()
        .and_then(|program| program.canonicalize())
        .map_err(InstallError::Binary)?;
    let (unit, file) = if home.dir() == Path::new(SYSTEM_HOME) {
        (Unit::SystemdSystem, PathBuf::from(SYSTEM_UNIT_FILE))
    } else {
        let plan = plan::plan(Target::User, platform, uid, env)?;
        (plan.unit, plan.file)
    };
    let on_disk = match std::fs::read_to_string(&file) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(source) => return Err(InstallError::Read { path: file, source }),
    };
    let found = compare(unit, on_disk.as_deref(), &Program::parse(&program)?)?;
    if found != Found::Older || unit != Unit::SystemdUser {
        return Ok((found, file));
    }
    let host = Host {
        platform,
        uid,
        program: &program,
        home,
        now: chrono::Utc::now(),
    };
    let file = install::install(Target::User, &host, env, |step| {
        run_step(step, STEP_DEADLINE)
    })?;
    Ok((Found::Same, file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sf_259_only_a_unit_file_that_starts_this_program_and_differs_is_older() {
        let program = Program::parse(Path::new("/opt/anima/bin/anima-daemon")).expect("a path");
        let other = Program::parse(Path::new("/home/me/build/anima-daemon")).expect("a path");
        for unit in [Unit::SystemdUser, Unit::SystemdSystem, Unit::LaunchAgent] {
            let current = render::render(unit, &program).expect("renders");
            let older = current.replacen('\n', "\n# an older line\n", 1);
            let others = render::render(unit, &other).expect("renders");
            assert_eq!(compare(unit, None, &program), Ok(Found::Absent));
            assert_eq!(compare(unit, Some(&current), &program), Ok(Found::Same));
            assert_eq!(compare(unit, Some(&older), &program), Ok(Found::Older));
            assert_eq!(compare(unit, Some(&others), &program), Ok(Found::Other));
            assert_eq!(compare(unit, Some(""), &program), Ok(Found::Other));
        }
    }
}

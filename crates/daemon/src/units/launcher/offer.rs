//! The I/O of the admin step: the question in the terminal and the run of the fixed commands
//! ([`super::install_steps`], [`super::uninstall_steps`]).

use std::io::{self, BufRead as _, IsTerminal as _, Write as _};
use std::path::Path;

use super::{ADMIN_DEADLINE, Elevation, PROGRAM_NAME, elevation, is_yes, shown};
use crate::model::tree::launcher;
use crate::units::step::{Step, run_step};

/// What the admin step did.
#[derive(Debug, PartialEq, Eq)]
pub enum Offer {
    /// This system does not need the launcher, or it is installed and current.
    NotNeeded,
    /// The commands ran and succeeded.
    Done,
    /// The person answered no.
    Declined,
    /// No terminal could ask the question, and the installer is not root.
    NoTerminal,
}

/// Why the admin step did not complete.
#[derive(Debug, thiserror::Error)]
pub enum OfferError {
    #[error("the model launcher is not beside the daemon program ({0})")]
    NoLauncher(String),
    #[error("the profile of the model launcher: {0}")]
    Profile(io::Error),
    #[error("{0} did not succeed")]
    Step(String),
    #[error("{0} did not complete: {1}")]
    Run(String, io::Error),
}

/// Installs the launcher beside `program` and its profile when this system needs them and they
/// are missing or outdated, after the question when the installer `uid` is not root.
///
/// # Errors
/// [`OfferError`] when the launcher file is missing or a command fails.
pub fn install(program: &Path, uid: u32) -> Result<Offer, OfferError> {
    if !launcher::is_used() || launcher::check().is_ok() {
        return Ok(Offer::NotNeeded);
    }
    let source = program.with_file_name(PROGRAM_NAME);
    if !source.is_file() {
        return Err(OfferError::NoLauncher(source.display().to_string()));
    }
    let Some(elevation) = elevation(uid, io::stdin().is_terminal()) else {
        return Ok(Offer::NoTerminal);
    };
    let text = model_launcher::profile::render(model_launcher::PATH)
        .ok_or_else(|| OfferError::Profile(io::ErrorKind::InvalidInput.into()))?;
    let mut rendered = tempfile::NamedTempFile::new().map_err(OfferError::Profile)?;
    rendered
        .write_all(text.as_bytes())
        .and_then(|()| rendered.flush())
        .map_err(OfferError::Profile)?;
    let steps = super::install_steps(elevation, &source, rendered.path());
    confirmed_run(elevation, &steps, super::QUESTION)
}

/// Removes the profile and the launcher when one of them is there, after the question when the
/// user `uid` is not root.
///
/// # Errors
/// [`OfferError`] when a command fails.
pub fn uninstall(uid: u32) -> Result<Offer, OfferError> {
    let there = |path: &str| std::fs::symlink_metadata(path).is_ok();
    if !there(model_launcher::profile::FILE) && !there(model_launcher::PATH) {
        return Ok(Offer::NotNeeded);
    }
    let Some(elevation) = elevation(uid, io::stdin().is_terminal()) else {
        return Ok(Offer::NoTerminal);
    };
    let steps = super::uninstall_steps(elevation);
    confirmed_run(
        elevation,
        &steps,
        "Remove the model launcher with sudo? [y/N] ",
    )
}

/// [`install`], with a line on standard error for each answer but [`Offer::NotNeeded`].
///
/// # Errors
/// As [`install`].
pub fn install_and_say(program: &Path, uid: u32) -> Result<(), OfferError> {
    match install(program, uid)? {
        Offer::NotNeeded => {}
        Offer::Done => eprintln!("Installed the model launcher at {}.", model_launcher::PATH),
        Offer::Declined | Offer::NoTerminal => eprintln!(
            "The model launcher is not installed: each model start here fails with containment_failed (launcher_missing). Run `anima-daemon install` in a terminal to install it."
        ),
    }
    Ok(())
}

/// [`uninstall`], with a line on standard error for each answer but [`Offer::NotNeeded`].
///
/// # Errors
/// As [`uninstall`].
pub fn uninstall_and_say(uid: u32) -> Result<(), OfferError> {
    match uninstall(uid)? {
        Offer::NotNeeded => {}
        Offer::Done => eprintln!("Removed the model launcher and its profile."),
        Offer::Declined | Offer::NoTerminal => eprintln!(
            "The model launcher and its profile stay: run `anima-daemon uninstall` in a terminal to remove them."
        ),
    }
    Ok(())
}

/// Shows `steps`, asks `question` for `sudo`, and runs them in order.
fn confirmed_run(
    elevation: Elevation,
    steps: &[Step],
    question: &str,
) -> Result<Offer, OfferError> {
    eprintln!("The model launcher needs these commands (AppArmor restricts user namespaces here):");
    for step in steps {
        eprintln!("  {}", shown(step));
    }
    if elevation == Elevation::Sudo {
        eprint!("{question}");
        let mut line = String::new();
        let read = io::stdin().lock().read_line(&mut line);
        if read.is_err() || !is_yes(&line) {
            return Ok(Offer::Declined);
        }
    }
    for step in steps {
        match run_step(step, ADMIN_DEADLINE) {
            Ok(true) => {}
            Ok(false) if step.may_fail => {}
            Ok(false) => return Err(OfferError::Step(shown(step))),
            Err(error) => return Err(OfferError::Run(shown(step), error)),
        }
    }
    Ok(Offer::Done)
}

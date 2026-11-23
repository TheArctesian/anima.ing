//! The admin step of the model launcher (contract K11 Section 9.1; task L1-6c; R-327): where
//! `AppArmor` restricts user namespaces, `anima-daemon install` puts the launcher and its profile
//! in place with three fixed commands, and `anima-daemon uninstall` takes them out. No shell
//! runs, and no argument comes from outside this program but the two paths it made.

use std::path::Path;
use std::time::Duration;

use model_launcher::profile;

use super::step::Step;

pub mod offer;
#[cfg(test)]
mod tests;

/// How long one admin command may take: `sudo` waits for a password.
pub const ADMIN_DEADLINE: Duration = Duration::from_secs(300);

/// The question of the terminal before the commands run.
pub const QUESTION: &str = "Install the model launcher with sudo? [y/N] ";

/// The name of the launcher program beside the daemon program.
pub const PROGRAM_NAME: &str = "anima-model-launcher";

const INSTALL: &str = "/usr/bin/install";
const PARSER: &str = "/usr/sbin/apparmor_parser";
const REMOVE: &str = "/usr/bin/rm";
const REMOVE_FOLDER: &str = "/usr/bin/rmdir";
const SUDO: &str = "/usr/bin/sudo";

/// How the commands get root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elevation {
    /// The installer is root (the system unit): the commands run as they are, with no question.
    Root,
    /// Through `sudo`, after `y` at the question of the terminal.
    Sudo,
}

/// The elevation of an install by the user `uid`: `None` when the user is not root and no
/// terminal can ask the question. No flag answers it for a script (R-327, C).
#[must_use]
pub const fn elevation(uid: u32, terminal: bool) -> Option<Elevation> {
    match (uid, terminal) {
        (0, _) => Some(Elevation::Root),
        (_, true) => Some(Elevation::Sudo),
        (_, false) => None,
    }
}

/// Whether the answer to [`QUESTION`] is yes: `y` or `yes`, in any case. Each other answer,
/// also an empty one, is no.
#[must_use]
pub fn is_yes(line: &str) -> bool {
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// The three commands that install the launcher `source` and the rendered profile `rendered` (a
/// private temporary file).
#[must_use]
pub fn install_steps(elevation: Elevation, source: &Path, rendered: &Path) -> Vec<Step> {
    let owned = ["-D", "-o", "root", "-g", "root", "-m"];
    let mut launcher: Vec<String> = owned.iter().map(|arg| (*arg).to_owned()).collect();
    launcher.extend([
        "0755".to_owned(),
        path_text(source),
        model_launcher::PATH.to_owned(),
    ]);
    let mut profile_file: Vec<String> = owned.iter().map(|arg| (*arg).to_owned()).collect();
    profile_file.extend([
        "0644".to_owned(),
        path_text(rendered),
        profile::FILE.to_owned(),
    ]);
    vec![
        step(elevation, INSTALL, launcher),
        step(elevation, INSTALL, profile_file),
        step(elevation, PARSER, words(&["-r", "-W", profile::FILE])),
    ]
}

/// The commands that take the profile and the launcher out: the profile is unloaded (which
/// fails when it is not loaded), then the two files and the folder of the launcher when it is
/// empty are removed.
#[must_use]
pub fn uninstall_steps(elevation: Elevation) -> Vec<Step> {
    let folder = Path::new(model_launcher::PATH)
        .parent()
        .map_or_else(String::new, path_text);
    let mut unload = step(elevation, PARSER, words(&["-R", profile::FILE]));
    unload.may_fail = true;
    vec![
        unload,
        step(
            elevation,
            REMOVE,
            words(&["-f", profile::FILE, model_launcher::PATH]),
        ),
        step(
            elevation,
            REMOVE_FOLDER,
            vec!["--ignore-fail-on-non-empty".to_owned(), folder],
        ),
    ]
}

/// The line of a step as the terminal shows it before the question.
#[must_use]
pub fn shown(step: &Step) -> String {
    std::iter::once(step.program.to_owned())
        .chain(step.args.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ")
}

fn step(elevation: Elevation, program: &'static str, args: Vec<String>) -> Step {
    match elevation {
        Elevation::Root => Step::new(program, &[]).with_args(args),
        Elevation::Sudo => Step::new(SUDO, &[])
            .with_args(std::iter::once(program.to_owned()).chain(args).collect()),
    }
}

fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

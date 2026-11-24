//! `anima-local-hub account`: the operator account, made on the hub computer only (F-4, F-8).
//! Secrets pass only through standard input and output; none is written, logged or put on a command line (LOG-007).

use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use anima_database::Role;

use crate::child::Spec;
use crate::members::{Carried, HUB};
use crate::state::{self, State};
use crate::{Folders, run};

#[cfg(all(test, unix))]
mod tests;

const NAME_LIMIT: usize = 64;

/// An operator's name; a first letter or digit keeps the hub from reading it as an option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "a name has 1 to {NAME_LIMIT} letters, digits, '.', '_' or '-', and starts with a letter or \
     a digit"
)]
pub struct NameError;

impl Name {
    /// # Errors
    /// [`NameError`] when `text` is not a name.
    pub fn parse(text: &str) -> Result<Self, NameError> {
        let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
        let first_is_plain = text
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric());
        if first_is_plain && text.len() <= NAME_LIMIT && text.chars().all(allowed) {
            Ok(Self(text.to_owned()))
        } else {
            Err(NameError)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// `anima-hub operator show`: one JSON line, the name and whether TOTP is set, or `null`.
    Status,
    /// `anima-hub operator totp <name> --new`: a new TOTP secret on standard output; it saves nothing.
    TotpNew(Name),
    /// `anima-hub operator create <name>`: password, secret and code on standard input; it makes or replaces the operator.
    Create(Name),
}

#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    /// The database runs only while the hub is ready.
    #[error("the hub of this folder is not ready: start it and wait for the state \"ready\"")]
    NotReady,
    #[error("anima-hub cannot be run ({0})")]
    Start(io::ErrorKind),
    /// Another user could have put a program there that would get the secrets (SEC-005).
    #[error(
        "{} is not a private folder or program of this user; the account command does not \
         run it",
        .0.display()
    )]
    Untrusted(PathBuf),
}

#[cfg(unix)]
const GROUP_OR_OTHERS: u32 = 0o077;
#[cfg(unix)]
const GROUP_OR_OTHERS_WRITE: u32 = 0o022;

/// Refuses the program unless it, the hub root and each folder between are this user's own, not symbolic links, and unchangeable by other users.
fn trusted(folders: &Folders, program: &Path) -> Result<(), AccountError> {
    #[cfg(unix)]
    let uid = nix::unistd::Uid::effective().as_raw();
    let untrusted = |path: &Path| AccountError::Untrusted(path.to_path_buf());
    let root = folders.root();
    let below = program.strip_prefix(root).map_err(|_| untrusted(program))?;
    let mut folder = root.to_path_buf();
    let mut folders_down = vec![folder.clone()];
    for part in below.parent().into_iter().flat_map(Path::components) {
        folder.push(part);
        folders_down.push(folder.clone());
    }
    for folder in &folders_down {
        let found = fs::symlink_metadata(folder).map_err(|_| untrusted(folder))?;
        #[cfg(unix)]
        let private = found.is_dir() && found.uid() == uid && found.mode() & GROUP_OR_OTHERS == 0;
        #[cfg(windows)]
        let private = found.is_dir()
            && anima_windows::files::directory(folder).is_ok()
            && anima_windows::files::secure(folder, true).unwrap_or(false);
        if !private {
            return Err(untrusted(folder));
        }
    }
    let found = fs::symlink_metadata(program).map_err(|_| untrusted(program))?;
    #[cfg(unix)]
    let trusted =
        found.is_file() && found.uid() == uid && found.mode() & GROUP_OR_OTHERS_WRITE == 0;
    #[cfg(windows)]
    let trusted = found.is_file() && anima_windows::files::read(program, false).is_ok();
    if !trusted {
        return Err(untrusted(program));
    }
    Ok(())
}

#[must_use]
fn command(folders: &Folders, action: &Action) -> Spec {
    let args: Vec<&str> = match action {
        Action::Status => vec!["operator", "show"],
        Action::TotpNew(Name(name)) => vec!["operator", "totp", name, "--new"],
        Action::Create(Name(name)) => vec!["operator", "create", name],
    };
    Spec {
        name: "anima-hub operator",
        program: Carried::installed_in(&folders.programs()).program(HUB),
        args: args.into_iter().map(Into::into).collect(),
        env: vec![
            (
                "ANIMA_DATABASE_URL_FILE",
                folders.database().url_file(Role::Hub).into(),
            ),
            ("ANIMA_KEY_FILE", folders.vault_key().into()),
        ],
    }
}

/// A `ready` state of a running `run` means the database runs.
#[must_use]
fn hub_is_ready(folders: &Folders) -> bool {
    run::is_running(folders)
        && matches!(
            state::read(&folders.state_file()),
            Some(State::Ready { .. })
        )
}

/// Runs the command of `action`; its standard error is dropped, and the caller tells a fixed message.
///
/// # Errors
/// [`AccountError::NotReady`] before anything runs when the hub is not ready,
/// [`AccountError::Untrusted`] when another user could have changed the program or a folder
/// above it, and [`AccountError::Start`] when the hub's program cannot be run.
pub fn run(
    folders: &Folders,
    action: &Action,
    input: Stdio,
    output: Stdio,
) -> Result<ExitStatus, AccountError> {
    if !hub_is_ready(folders) {
        return Err(AccountError::NotReady);
    }
    let spec = command(folders, action);
    trusted(folders, &spec.program)?;
    let mut command = Command::new(&spec.program);
    command.env_clear();
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    command
        .args(&spec.args)
        .envs(spec.env.iter().map(|(key, value)| (key, value)))
        .current_dir(folders.root())
        .stdin(input)
        .stdout(output)
        .stderr(Stdio::null())
        .status()
        .map_err(|error| AccountError::Start(error.kind()))
}

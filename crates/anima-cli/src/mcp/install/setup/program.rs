use super::super::access::Others;
#[cfg(windows)]
use super::super::access::Writer;
#[cfg(unix)]
use super::super::access::{Groups, replaceable};
use super::Here;
#[cfg(unix)]
use super::script;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Why `anima` cannot be the program of an entry (MCP-033).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    #[error("the path of anima is not an absolute path of plain text")]
    NotAbsolute,
    #[error("the path of anima is not a program that can be run")]
    NotAProgram,
    #[error("a different user can replace the anima program or its folder: {0}")]
    Replaceable(Others),
}

/// The path of `anima` after [`check_command`] accepted it: the only value an entry is made of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked(PathBuf);

impl Checked {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// Checks that `anima` is a fixed program that only this user or the system can replace, and
/// that its path is text that no agent reads as a variable (MCP-033).
///
/// # Errors
/// [`CommandError`] when it is not absolute, not a runnable regular file, or replaceable.
pub fn check_command(anima: &Path, user: u32) -> Result<Checked, CommandError> {
    let text = anima.to_str().ok_or(CommandError::NotAbsolute)?;
    // `$` and braces are variables in the files of some agents; `..` names a different place.
    let plain = !text
        .chars()
        .any(|character| character.is_control() || matches!(character, '$' | '{' | '}'));
    if !anima.is_absolute()
        || !plain
        || anima
            .components()
            .any(|part| part == std::path::Component::ParentDir)
    {
        return Err(CommandError::NotAbsolute);
    }
    let program = fs::metadata(anima).map_err(|_| CommandError::NotAProgram)?;
    if !runnable(anima, &program) {
        return Err(CommandError::NotAProgram);
    }
    // The program may be a link that an installer made (INS-002): each link and folder is checked.
    #[cfg(unix)]
    let others = replaceable(anima, user, &Groups::system());
    #[cfg(windows)]
    let others = {
        let real = fs::canonicalize(anima).map_err(|_| CommandError::NotAProgram)?;
        replaceable(anima, user).or_else(|| replaceable(&real, user))
    };
    match others {
        Some(others) => Err(CommandError::Replaceable(others)),
        None => Ok(Checked(anima.to_path_buf())),
    }
}

/// An agent's own program on `PATH`. It runs only when no different user can replace it, a link
/// to it, a folder above, or the interpreter of a script (MCP-033).
pub(super) enum Found {
    Fixed(PathBuf),
    Replaceable { program: PathBuf, others: Others },
    Missing,
}

/// The first program named `name` on `PATH`, if it is one that may be run (MCP-035).
pub(super) fn fixed_program(here: &Here, name: &str) -> Found {
    let Some(path) = program(here, name) else {
        return Found::Missing;
    };
    #[cfg(unix)]
    let others =
        replaceable(&path, here.user, &here.groups).or_else(|| script::interpreter(here, &path));
    #[cfg(windows)]
    let others = {
        let real = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        replaceable(&path, here.user).or_else(|| replaceable(&real, here.user))
    };
    match others {
        Some(others) => Found::Replaceable {
            program: path,
            others,
        },
        None => Found::Fixed(path),
    }
}

pub(super) fn program(here: &Here, name: &str) -> Option<PathBuf> {
    here.path
        .iter()
        .filter(|folder| folder.is_absolute())
        .flat_map(|folder| {
            #[cfg(unix)]
            let names = vec![folder.join(name)];
            #[cfg(windows)]
            let names = ["exe", "com", "cmd", "bat"]
                .map(|extension| folder.join(format!("{name}.{extension}")));
            names
        })
        .find(|path| fs::metadata(path).is_ok_and(|metadata| runnable(path, &metadata)))
}

#[cfg(windows)]
fn replaceable(path: &Path, _: u32) -> Option<Others> {
    path.ancestors()
        .find(|part| !anima_windows::files::secure(part, false).unwrap_or(false))
        .map(|part| Others {
            path: part.to_path_buf(),
            writer: Writer::AccessList,
        })
}
pub(super) fn runnable(path: &Path, metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        let _ = path;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(windows)]
    {
        metadata.is_file()
            && path
                .extension()
                .and_then(|part| part.to_str())
                .is_some_and(|part| {
                    ["exe", "com", "cmd", "bat"]
                        .iter()
                        .any(|extension| part.eq_ignore_ascii_case(extension))
                })
    }
}

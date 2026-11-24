//! A read of the repository for the Git engine (TH116): the one run that each thread of the
//! runner may have, also a confined one, in the confinement of that thread.
//!
//! The launcher does not take the word of the engine that a run is a read: the run must have
//! the one shape of [`crate::threads::git::place::shared::pack::is_read`], which looks at the
//! whole run, and the launcher checks the input of a pack again itself ([`listed`]). A read
//! gets no environment of Git from the login: only what `is_read` allows.
//!
//! A Codex thread that is not in Full access reads inside `codex sandbox` with the profile of
//! the folder that its turns have ([`confined`]). A Claude Code thread and a thread in Full
//! access read with the rights of the person, as their agents run.
//!
//! I/O: [`listed`] reads the input file of a pack; [`codex_home`] makes and reads a folder.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, Read as _, Seek as _};
use std::path::{Path, PathBuf};

use agent_threads::drive::{Mode, Program};
use uuid::Uuid;

use super::denied;

/// The most bytes of the input of a pack: a list of object names.
const MOST_INPUT: u64 = 16 * 1024 * 1024;

/// A full object name of SHA-1 or SHA-256, in lower case.
fn object_name(name: &str) -> bool {
    matches!(name.len(), 40 | 64)
        && name
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Opens the input of a pack with no link followed and checks it: each line an object name, or
/// `^` and an object name. The file that is checked is the file that the program reads.
///
/// # Errors
/// The I/O error, or `PermissionDenied` for a list with any other line.
pub fn listed(file: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    // No link is followed at the file itself; the folders above it are the records of the
    // runner, mode 0700.
    let mut opened = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
        .open(file)?;
    let mut text = String::new();
    (&mut opened)
        .take(MOST_INPUT + 1)
        .read_to_string(&mut text)?;
    let fits = u64::try_from(text.len()).is_ok_and(|len| len <= MOST_INPUT);
    let names = text
        .lines()
        .all(|line| object_name(line.strip_prefix('^').unwrap_or(line)));
    if !fits || !names {
        return Err(denied("the input of a pack holds only object names"));
    }
    opened.rewind()?;
    Ok(opened)
}

/// How a thread reads: its mode, its program and, for a confined Codex thread, its copy of
/// Codex and the folder that its profile keeps read-only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Confinement {
    pub mode: Mode,
    pub program: Program,
    /// The copy of Codex that started a Codex thread, when the runner knows it.
    pub codex: Option<PathBuf>,
    /// The folder of Anima that the thread folder holds, kept read-only (THR-041).
    pub kept: Option<PathBuf>,
}

/// The program and the arguments of a read of `thread` in `dir`: `git` itself with the rights of
/// the person, or `git` inside `codex sandbox` for a Codex thread that is not in Full access.
/// The second is `true` when the read needs the home of Codex of [`codex_home`].
///
/// The profile is given with `-P` and its whole content with `-c`. Codex merges a table of the
/// same name from each file of configuration into it (measured with 0.170 on 2026-11-23), so
/// the read runs with a home of Codex that holds no file of configuration ([`codex_home`]):
/// then no file of the user is read, and no project is trusted, so no file of a project is read.
///
/// # Errors
/// `PermissionDenied` for a Codex thread whose copy of Codex is not known, or whose mode has no
/// sandbox of the folder.
pub fn confined(
    thread: Uuid,
    seat: &Confinement,
    (git, dir, args): (&Path, &Path, &[OsString]),
) -> io::Result<(PathBuf, Vec<OsString>, bool)> {
    if seat.program != Program::Codex || seat.mode == Mode::FullAccess {
        return Ok((git.to_owned(), args.to_vec(), false));
    }
    let Some(codex) = &seat.codex else {
        return Err(denied("the copy of Codex of the thread is not known"));
    };
    if !seat.mode.folder_sandbox() {
        return Err(denied(
            "a read of a Codex thread in this mode has no sandbox",
        ));
    }
    let profile = format!("anima-reads-{}", thread.as_simple());
    let mut wrapped: Vec<OsString> = vec!["sandbox".into(), "-P".into(), profile.clone().into()];
    let mut add = |setting: String| wrapped.extend(["-c".into(), setting.into()]);
    add(format!("permissions.{profile}.extends=\":workspace\""));
    if let Some(kept) = &seat.kept {
        let kept = toml_string(kept)?;
        add(format!(
            "permissions.{profile}.filesystem={{{kept}=\"read\"}}"
        ));
    }
    wrapped.extend(["-C".into(), dir.as_os_str().to_owned(), "--".into()]);
    wrapped.push(git.as_os_str().to_owned());
    wrapped.extend(args.iter().cloned());
    Ok((codex.clone(), wrapped, true))
}

/// The home of Codex for the reads: a folder of the runner, mode 0700, made when it is not
/// there. It may hold only the folder `tmp` that Codex writes; a folder with anything else (a
/// file of configuration put there) is refused.
///
/// # Errors
/// The I/O error, or `PermissionDenied` for a folder that is a link or holds more than `tmp`.
pub fn codex_home(home: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _};
    match std::fs::DirBuilder::new().mode(0o700).create(home) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let made = std::fs::symlink_metadata(home)?;
    let own = made.is_dir() && made.uid() == rustix::process::getuid().as_raw();
    if !own || made.mode() & 0o077 != 0 {
        return Err(denied(
            "the home of Codex for reads is not a private folder",
        ));
    }
    for entry in std::fs::read_dir(home)? {
        if entry?.file_name() != "tmp" {
            return Err(denied("the home of Codex for reads holds a file"));
        }
    }
    Ok(())
}

/// A path as a TOML string: the path must be UTF-8 with no quote, backslash or control.
fn toml_string(path: &Path) -> io::Result<String> {
    let text = path.to_str().unwrap_or("\"");
    if text
        .chars()
        .any(|c| c == '"' || c == '\\' || c.is_control())
    {
        return Err(denied(
            "the kept folder has a name that a profile cannot hold",
        ));
    }
    Ok(format!("\"{text}\""))
}

/// Whether a name of the login environment goes to a read: no name of Anima (THR-040), no name
/// of Git (also `GIT_CONFIG_*`), so that the login cannot point the read at a different
/// repository or program, and no home of Codex.
#[must_use]
pub fn login_env(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    !name.starts_with("ANIMA_") && !name.starts_with("GIT_") && name != "CODEX_HOME"
}

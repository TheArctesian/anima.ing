//! A read of the repository for the Git engine (TH116): the one run that each thread of the
//! runner may have, also a confined one, in the confinement of that thread.
//!
//! The launcher does not take the word of the engine that a run is a read. [`shape`] checks the
//! whole run: the `git` of the engine, the closed arguments of [`CLOSED`] first, then only
//! `rev-list` with full object names and one `--not`, or `pack-objects` that writes to the one folder of the thread
//! in its worktree, with a list of object names as its input. Each other run is a hook, which
//! only a thread in Full access may have. A read gets no environment from the engine: the
//! launcher sets [`QUIET`] itself.
//!
//! A Codex thread that is not in Full access reads inside `codex sandbox` with the profile of
//! the folder that its turns have ([`confined`]). A Claude Code thread and a thread in Full
//! access read with the rights of the person, as their agents run.
//!
//! I/O: [`listed`] reads the input file of a pack.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, Read as _, Seek as _};
use std::path::{Path, PathBuf};

use agent_threads::drive::{Mode, Program};
use uuid::Uuid;

use super::denied;
use crate::threads::git::run::hooks::ThreadRun;

/// What each read gets before its own arguments, where it wins over each file of configuration:
/// no fetch of a missing object, no transport, no `ssh` command, no monitor program, no hook.
pub const CLOSED: [&str; 22] = [
    "--no-lazy-fetch",
    "--no-optional-locks",
    "--no-pager",
    "-c",
    "protocol.allow=never",
    "-c",
    "core.sshCommand=false",
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "core.alternateRefsCommand=",
    "--no-replace-objects",
    "-c",
    "core.compression=-1",
    "-c",
    "pack.compression=-1",
    "-c",
    "pack.window=10",
    "-c",
    "pack.depth=50",
];

/// The environment that the launcher adds to each read: no fetch of a missing object, no prompt.
pub const QUIET: [(&str, &str); 3] = [
    ("GIT_NO_LAZY_FETCH", "1"),
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_SSH_COMMAND", "false"),
];

/// The start of the name of the folder in the worktree that a pack is written to.
const PACK_FOLDER: &str = ".anima-pack-";

/// The most bytes of the input of a pack: a list of object names.
const MOST_INPUT: u64 = 16 * 1024 * 1024;

/// The two reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reading {
    /// `rev-list --objects --quiet`: whether each object is there.
    Objects,
    /// `pack-objects --revs --quiet`: one pack into the folder of the thread.
    Pack,
}

/// The read that `run` is, or the reason that it is none. `git` is the program of the engine.
///
/// # Errors
/// `PermissionDenied` with the reason, for each run that is not exactly one of the two reads.
pub fn shape(run: &ThreadRun, git: Option<&Path>) -> io::Result<Reading> {
    if git != Some(run.program.as_path()) {
        return Err(denied("a read runs only the git of the engine"));
    }
    if !run.env.is_empty() {
        return Err(denied("a read gets no environment from the engine"));
    }
    let Some(own) = run.args.strip_prefix(CLOSED.map(OsString::from).as_slice()) else {
        return Err(denied("a read starts with the closed arguments"));
    };
    let own: Vec<&str> = own.iter().map(|arg| arg.to_str().unwrap_or("")).collect();
    match own.as_slice() {
        // No `--end-of-options`: git 2.54 then takes `--not` for a name. Each name is checked
        // to be an object name, so none can be an option.
        ["rev-list", "--objects", "--quiet", names @ ..] => objects(names, run.input.as_deref()),
        ["pack-objects", "--revs", "--quiet", base] => pack(run, base),
        _ => Err(denied(
            "a read is rev-list or pack-objects, with nothing else",
        )),
    }
}

/// `rev-list`: object names, `--not` one time, more object names, no input.
fn objects(names: &[&str], input: Option<&Path>) -> io::Result<Reading> {
    let Some(not) = names.iter().position(|name| *name == "--not") else {
        return Err(denied(
            "rev-list names the history that is known with --not",
        ));
    };
    let (wanted, known) = (&names[..not], &names[not + 1..]);
    let hashes = |list: &[&str]| list.iter().all(|name| object_name(name));
    if wanted.is_empty() || !hashes(wanted) || !hashes(known) || input.is_some() {
        return Err(denied("rev-list reads only object names"));
    }
    Ok(Reading::Objects)
}

/// `pack-objects`: the pack goes to the folder of the thread in its worktree, and the input is
/// a list of object names.
fn pack(run: &ThreadRun, base: &str) -> io::Result<Reading> {
    let folder = format!("{PACK_FOLDER}{}", run.thread.as_simple());
    if Path::new(base) != run.dir.join(folder).join("p") || run.input.is_none() {
        return Err(denied(
            "pack-objects writes only to the pack folder of the thread",
        ));
    }
    Ok(Reading::Pack)
}

/// A full object name of SHA-1 or SHA-256, in lower case.
fn object_name(name: &str) -> bool {
    matches!(name.len(), 40 | 64)
        && name
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Opens the input of a pack and checks it: each line an object name, or `^` and an object
/// name. The file that is checked is the file that the program reads.
///
/// # Errors
/// The I/O error, or `PermissionDenied` for a list with any other line.
pub fn listed(file: &Path) -> io::Result<File> {
    let mut opened = File::open(file)?;
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
    if seat.program == Program::ClaudeCode || seat.mode == Mode::FullAccess {
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

/// The login environment of a read: no name of Anima (THR-040) and no name of Git, so that
/// the login cannot point the read at a different repository or program.
#[must_use]
pub fn login_env(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    !name.starts_with("ANIMA_") && !name.starts_with("GIT_")
}

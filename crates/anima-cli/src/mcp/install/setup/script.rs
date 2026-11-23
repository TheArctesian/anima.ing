//! The folders of `PATH` that an agent's program runs with (MCP-035), and the interpreter that the
//! program names when it is a script (`#!` on its first line): who other than the person and root
//! can change them (MCP-033). An agent installed with npm is a script that `env` starts with the
//! `node` it finds on `PATH`; a version manager's `node` can be a script again.

use std::fs;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};

use nix::unistd::{AccessFlags, access};

use super::super::access::{Others, Writer, creatable, replaceable};
use super::Here;

/// The most of the first line that the kernel reads; the rest is not part of the interpreter.
const FIRST_LINE: u64 = 512;

/// The most scripts that start one another; one more counts as not known.
const MOST_NESTED: usize = 4;

/// The options of `env` that change neither where it looks for the program nor which word names
/// it: `-S` splits the rest of the line into words, as this reading does. `-i` is not one: with no
/// `PATH`, `env` looks in folders of its own.
const ENV_OPTIONS: [&str; 3] = ["--", "-S", "-v"];

/// The folders of `PATH` that the agent's program runs with (MCP-035), as absolute paths: each one
/// in which no user other than the person and root can replace a program or make one, in the order
/// of `PATH`. A folder that is not there counts by the nearest folder above it that is, since a
/// user who can write that can make it. The others are left out: neither `env` nor the program
/// looks there.
pub(super) fn folders(here: &Here) -> Vec<PathBuf> {
    here.path
        .iter()
        // A relative folder is checked from the folder the program runs in, the home folder, and
        // given as an absolute path: a program that changes its folder must not find a different one.
        .map(|folder| here.home.join(folder))
        .filter(|folder| {
            let there = folder
                .ancestors()
                .find(|part| fs::symlink_metadata(part).is_ok())
                .unwrap_or(folder);
            replaceable(there, here.user, &here.groups).is_none()
                && creatable(there, here.user, &here.groups).is_none()
        })
        .collect()
}

/// Who other than the person and root can change the interpreter of the script at `program`: the
/// interpreter that its first line names, or with `env` the program that `env` finds in the
/// [`folders`] of `PATH`, and so on while that is a script again. `None` when no one can, or
/// `program` is not a script.
pub(super) fn interpreter(here: &Here, program: &Path) -> Option<Others> {
    let mut program = program.to_path_buf();
    for _ in 0..=MOST_NESTED {
        match started(here, &program) {
            Ok(None) => return None,
            Ok(Some(next)) => program = next,
            Err(others) => return Some(others),
        }
    }
    Some(unknown(program))
}

/// The program that the script at `program` starts, `None` when it is not a script, or `Err`
/// with who can change that program, or a line that the check does not read as the system does.
fn started(here: &Here, program: &Path) -> Result<Option<PathBuf>, Others> {
    let line = match first_line(program) {
        Ok(Some(line)) => line,
        Ok(None) => return Ok(None),
        Err(_) => return Err(unknown(program.to_path_buf())),
    };
    // The kernel splits the line at spaces and tabs only; a different blank or a control
    // character (as the `\r` of a line of Windows) is part of a name.
    let words: Vec<&str> = line
        .split([' ', '\t'])
        .filter(|word| !word.is_empty())
        .collect();
    let odd = words
        .iter()
        .any(|word| word.contains(|part: char| part.is_control() || part.is_whitespace()));
    let [first, rest @ ..] = words.as_slice() else {
        return Err(unknown(program.to_path_buf()));
    };
    if odd {
        return Err(unknown(program.to_path_buf()));
    }
    // A relative interpreter is found from the folder the program runs in: the home folder.
    let named = here.home.join(first);
    runs(here, &named)?;
    if named.file_name().is_none_or(|name| name != "env") {
        return Ok(Some(named));
    }
    // Linux gives `env` the rest of the line as one word, which only `-S` splits.
    let one_word = cfg!(target_os = "linux") && rest.len() > 1 && rest[0] != "-S";
    let Some(name) = env_program(rest.iter().copied()).filter(|_| !one_word) else {
        return Err(unknown(named));
    };
    if name.contains('/') {
        let path = here.home.join(name);
        runs(here, &path)?;
        Ok(Some(path))
    } else {
        on_path(here, name).map(Some)
    }
}

/// `Err` when a user other than the person and root can change the program at `path`, or when it
/// is not a regular file that the person can run: the system would not start it, and `env` would
/// go on to a different one.
fn runs(here: &Here, path: &Path) -> Result<(), Others> {
    if let Some(others) = replaceable(path, here.user, &here.groups) {
        return Err(others);
    }
    if runnable(path) {
        Ok(())
    } else {
        Err(unknown(path.to_path_buf()))
    }
}

/// Whether `path` is a regular file that the person can run.
fn runnable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
        && access(path, AccessFlags::X_OK).is_ok()
}

/// The program that `env` runs, from the words after `env` on the first line: the first word that
/// is not an option or an assignment. `None` when there is none, or when an option, a new `PATH`,
/// or a quote, an escape or a variable (which `env -S` reads) could change which program `env`
/// finds.
fn env_program<'a>(words: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    for word in words {
        if word.starts_with("PATH=") || word.contains(['"', '\'', '\\', '$']) {
            return None;
        } else if word.starts_with('-') {
            ENV_OPTIONS.contains(&word).then_some(())?;
        } else if !word.contains('=') {
            return Some(word);
        }
    }
    None
}

/// The text after `#!` on the first line of the file at `program`: `None` for a file that is not a
/// script, an error when it cannot be read, since the system can still start it.
fn first_line(program: &Path) -> io::Result<Option<String>> {
    // Only a regular file: opening a pipe would wait for a writer.
    if !fs::metadata(program)?.is_file() {
        return Ok(None);
    }
    let mut head = Vec::new();
    fs::File::open(program)?
        .take(FIRST_LINE)
        .read_to_end(&mut head)?;
    Ok(head.strip_prefix(b"#!").map(|line| {
        let line = line.split(|byte| *byte == b'\n').next().unwrap_or_default();
        String::from_utf8_lossy(line).into_owned()
    }))
}

/// The program `name` that `env` finds in the [`folders`] of `PATH`: the first file of that name
/// that the person can run. `Err` names who other than the person and root can change it, or an
/// entry of that name before it, which `env` tries first. A program that is not found counts as not
/// known.
fn on_path(here: &Here, name: &str) -> Result<PathBuf, Others> {
    for folder in folders(here) {
        let found = folder.join(name);
        // Each entry of the name, also a link that leads nowhere yet and one that cannot be run.
        if fs::symlink_metadata(&found).is_err() {
            continue;
        }
        if let Some(others) = replaceable(&found, here.user, &here.groups) {
            return Err(others);
        }
        if runnable(&found) {
            return Ok(found);
        }
    }
    Err(unknown(PathBuf::from(name)))
}

/// `path`, of which who can change it is not known.
fn unknown(path: PathBuf) -> Others {
    Others {
        path,
        writer: Writer::Unknown,
    }
}

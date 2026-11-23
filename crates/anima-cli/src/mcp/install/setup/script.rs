//! Who other than the person and root can change the `PATH` folders (MCP-035) and the script interpreters (MCP-033) of an agent's program.
//! An npm agent is a script that `env` starts with the `node` on `PATH`, which can be a script again.

use std::fs;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};

use nix::unistd::{AccessFlags, access};

use super::super::access::{Others, Writer, creatable, replaceable};
use super::Here;

/// The most of the first line that the kernel reads.
const FIRST_LINE: u64 = 512;

const MOST_NESTED: usize = 4;

/// `env` options that change neither where it looks nor which word names the program; `-i` drops `PATH`, so it is not one.
const ENV_OPTIONS: [&str; 3] = ["--", "-S", "-v"];

/// Only folders where no user but the person and root can replace or make a program, in `PATH` order.
/// A missing folder counts by the nearest folder above it, since a user who can write that can make it (MCP-035).
pub(super) fn folders(here: &Here) -> Vec<PathBuf> {
    here.path
        .iter()
        // Absolute from the home folder, so a program that changes its folder finds the same one.
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

fn started(here: &Here, program: &Path) -> Result<Option<PathBuf>, Others> {
    let line = match first_line(program) {
        Ok(Some(line)) => line,
        Ok(None) => return Ok(None),
        Err(_) => return Err(unknown(program.to_path_buf())),
    };
    // The kernel splits the line at spaces and tabs only; any other blank is part of a name.
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

/// A program that is not a runnable regular file counts as not known: `env` would go on to a different one.
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

fn runnable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
        && access(path, AccessFlags::X_OK).is_ok()
}

/// `None` when an option, a new `PATH`, a quote, an escape or a variable (read by `env -S`) could change which program `env` finds.
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

/// An unreadable file is an error, not `None`: the system can still start it.
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

/// An earlier entry of the same name that another user can change also counts: `env` tries it first.
fn on_path(here: &Here, name: &str) -> Result<PathBuf, Others> {
    for folder in folders(here) {
        let found = folder.join(name);
        // Also a link that leads nowhere yet and an entry that cannot be run.
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

fn unknown(path: PathBuf) -> Others {
    Others {
        path,
        writer: Writer::Unknown,
    }
}

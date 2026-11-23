//! The interpreter that an agent's program names when it is a script (`#!` on its first line), and
//! who other than the person and root can change it (MCP-033). An agent installed with npm is a
//! script that `env` starts with the `node` it finds on `PATH`; a version manager's `node` can be
//! a script again.

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use super::super::access::{Others, Writer, replaceable};
use super::Here;
use super::program::runnable;

/// The most of the first line that the kernel reads; the rest is not part of the interpreter.
const FIRST_LINE: u64 = 512;

/// The most scripts that start one another; one more counts as not known.
const MOST_NESTED: usize = 4;

/// The options of `env` that change neither where it looks for the program nor which word names
/// it: `-S` splits the rest of the line into words, as this reading does.
const ENV_OPTIONS: [&str; 6] = ["-", "--", "-i", "--ignore-environment", "-S", "-v"];

/// Who other than the person and root can change the interpreter of the script at `program`: the
/// interpreter that its first line names, or with `env` the program that `env` finds on `PATH`,
/// and so on while that is a script again. `None` when no one can, or `program` is not a script.
pub(super) fn interpreter(here: &Here, program: &Path) -> Option<Others> {
    let mut program = program.to_path_buf();
    for _ in 0..MOST_NESTED {
        let line = first_line(&program)?;
        let mut words = line.split_whitespace();
        // A relative interpreter is found from the folder the program runs in: the home folder.
        let named = here.home.join(words.next()?);
        if let Some(others) = replaceable(&named, here.user, &here.groups) {
            return Some(others);
        }
        if named.file_name().is_none_or(|name| name != "env") {
            program = named;
            continue;
        }
        let Some(name) = env_program(words) else {
            return Some(Others {
                path: named,
                writer: Writer::Unknown,
            });
        };
        program = if name.contains('/') {
            let path = here.home.join(name);
            if let Some(others) = replaceable(&path, here.user, &here.groups) {
                return Some(others);
            }
            path
        } else {
            match on_path(here, name) {
                Ok(Some(found)) => found,
                Ok(None) => return None,
                Err(others) => return Some(others),
            }
        };
    }
    first_line(&program).map(|_| Others {
        path: program,
        writer: Writer::Unknown,
    })
}

/// The program that `env` runs, from the words after `env` on the first line: the first word that
/// is not an option or an assignment. `None` when there is none, or when an option or a new
/// `PATH` could change which program `env` finds (as `-P` or `-u NAME`).
fn env_program<'a>(words: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    for word in words {
        if word.starts_with("PATH=") {
            return None;
        } else if word.starts_with('-') {
            ENV_OPTIONS.contains(&word).then_some(())?;
        } else if !word.contains('=') {
            return Some(word);
        }
    }
    None
}

/// The text after `#!` on the first line of the file at `program`, or `None` for a file that is
/// not a script.
fn first_line(program: &Path) -> Option<String> {
    // Only a regular file: opening a pipe would wait for a writer.
    if !fs::metadata(program).is_ok_and(|metadata| metadata.is_file()) {
        return None;
    }
    let mut head = Vec::new();
    fs::File::open(program)
        .and_then(|file| file.take(FIRST_LINE).read_to_end(&mut head))
        .ok()?;
    let line = head.strip_prefix(b"#!")?;
    let line = line.split(|byte| *byte == b'\n').next().unwrap_or_default();
    Some(String::from_utf8_lossy(line).into_owned())
}

/// The program `name` that `env` finds on `PATH`, when no user but the person and root can change
/// it or a folder that `env` looks in before it: `Err` names who can. A folder that is not there
/// counts by the nearest folder above it that is, since a user who can write that can make it.
fn on_path(here: &Here, name: &str) -> Result<Option<PathBuf>, Others> {
    for folder in &here.path {
        let folder = here.home.join(folder);
        let there = folder
            .ancestors()
            .find(|part| fs::symlink_metadata(part).is_ok())
            .unwrap_or(&folder);
        if let Some(others) = replaceable(there, here.user, &here.groups) {
            return Err(others);
        }
        let found = folder.join(name);
        if fs::metadata(&found).is_ok_and(|metadata| runnable(&found, &metadata)) {
            return replaceable(&found, here.user, &here.groups).map_or(Ok(Some(found)), Err);
        }
    }
    Ok(None)
}

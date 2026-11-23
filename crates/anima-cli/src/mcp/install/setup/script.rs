//! The interpreter that an agent's program names when it is a script (`#!` on its first line), and
//! who other than the person and root can change it (MCP-033), also through the folders of `PATH`
//! that `env` looks in (MCP-035). An agent installed with npm is a script that `env` starts with
//! the `node` it finds on `PATH`; a version manager's `node` can be a script again.

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

/// Who other than the person and root can change the interpreter of the script at `program`: the
/// interpreter that its first line names, or with `env` the program that `env` finds on `PATH`,
/// and so on while that is a script again. `None` when no one can, or `program` is not a script.
pub(super) fn interpreter(here: &Here, program: &Path) -> Option<Others> {
    let mut program = program.to_path_buf();
    for _ in 0..MOST_NESTED {
        let line = match first_line(&program) {
            Ok(Some(line)) => line,
            Ok(None) => return None,
            Err(_) => return Some(unknown(program)),
        };
        let mut words = line.split_whitespace();
        let Some(first) = words.next() else {
            return Some(unknown(program));
        };
        // A relative interpreter is found from the folder the program runs in: the home folder.
        let named = here.home.join(first);
        if let Some(others) = replaceable(&named, here.user, &here.groups) {
            return Some(others);
        }
        if named.file_name().is_none_or(|name| name != "env") {
            program = named;
            continue;
        }
        let Some(name) = env_program(words) else {
            return Some(unknown(named));
        };
        program = if name.contains('/') {
            let path = here.home.join(name);
            if let Some(others) = replaceable(&path, here.user, &here.groups) {
                return Some(others);
            }
            path
        } else {
            match on_path(here, name) {
                Ok(found) => found,
                Err(others) => return Some(others),
            }
        };
    }
    match first_line(&program) {
        Ok(None) => None,
        _ => Some(unknown(program)),
    }
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

/// The program `name` that `env` finds on `PATH`: the first file of that name that the person can
/// run. `Err` names who other than the person and root can change it, an entry of that name before
/// it, or a folder that `env` looks in up to it, where they could also make a new entry (the
/// sticky bit does not stop that). A folder that is not there counts by the nearest folder above
/// it that is, since a user who can write that can make it. A program that is not found counts as
/// not known.
fn on_path(here: &Here, name: &str) -> Result<PathBuf, Others> {
    for folder in &here.path {
        // A relative folder is found from the folder the program runs in: the home folder.
        let folder = here.home.join(folder);
        let there = folder
            .ancestors()
            .find(|part| fs::symlink_metadata(part).is_ok())
            .unwrap_or(&folder);
        let open = replaceable(there, here.user, &here.groups)
            .or_else(|| creatable(there, here.user, &here.groups));
        if let Some(others) = open {
            return Err(others);
        }
        let found = folder.join(name);
        // Each entry of the name, also a link that leads nowhere yet and one that cannot be run.
        if fs::symlink_metadata(&found).is_err() {
            continue;
        }
        if let Some(others) = replaceable(&found, here.user, &here.groups) {
            return Err(others);
        }
        if fs::metadata(&found).is_ok_and(|metadata| metadata.is_file())
            && access(&found, AccessFlags::X_OK).is_ok()
        {
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

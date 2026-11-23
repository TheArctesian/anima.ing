//! The commands and the skills of Claude Code in a folder before any thread runs there (TH117):
//! read from the files of `.claude/commands` and `.claude/skills` of the folder and of the home
//! folder, as Claude Code reads them. Only the start of each file is read, a link or a file that
//! is not a plain file is never opened, and the walk stops at a fixed depth and count, so a
//! hostile folder cannot make the runner read much or wait.

use std::io::Read as _;
use std::path::Path;

use agent_threads::drive::{CommandRow, MOST_ROWS, claude_file};

/// The most bytes of one file that are read: the frontmatter and the first line.
const MOST_READ: u64 = 8 * 1024;
/// The most folders below `.claude/commands` that a name may have.
const MOST_DEPTH: usize = 3;
/// The most entries of one folder that the walk looks at.
const MOST_ENTRIES: usize = 500;

/// The commands of the folder, then those of the home folder that the folder does not have.
#[must_use]
pub fn commands(folder: &Path, home: Option<&Path>) -> Vec<CommandRow> {
    let mut rows: Vec<CommandRow> = Vec::new();
    for root in std::iter::once(folder).chain(home) {
        let claude = root.join(".claude");
        let mut found = Vec::new();
        walk(&claude.join("commands"), "", 0, &mut found);
        skills(&claude.join("skills"), &mut found);
        for row in found {
            if rows.len() < MOST_ROWS && !rows.iter().any(|each| each.name == row.name) {
                rows.push(row);
            }
        }
    }
    rows
}

/// The `.md` files below `folder`, named `prefix:name`.
fn walk(folder: &Path, prefix: &str, depth: usize, found: &mut Vec<CommandRow>) {
    for (name, path, is_dir) in entries(folder) {
        if found.len() >= MOST_ROWS {
            return;
        }
        let named = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}:{name}")
        };
        if is_dir && depth < MOST_DEPTH {
            walk(&path, &named, depth + 1, found);
        } else if let Some(command) = named.strip_suffix(".md").filter(|_| !is_dir) {
            found.extend(start_of(&path).and_then(|text| claude_file(command, &text)));
        }
    }
}

/// Each `<name>/SKILL.md` of `folder`.
fn skills(folder: &Path, found: &mut Vec<CommandRow>) {
    for (name, path, is_dir) in entries(folder) {
        if found.len() >= MOST_ROWS {
            return;
        }
        if is_dir {
            found.extend(
                start_of(&path.join("SKILL.md")).and_then(|text| claude_file(&name, &text)),
            );
        }
    }
}

/// The plain files and folders of `folder`, by name, with no link; in the order of their names.
fn entries(folder: &Path) -> Vec<(String, std::path::PathBuf, bool)> {
    let Ok(read) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut entries: Vec<_> = read
        .take(MOST_ENTRIES)
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let kind = entry.file_type().ok()?;
            let name = entry.file_name().into_string().ok()?;
            (kind.is_dir() || kind.is_file()).then(|| (name, entry.path(), kind.is_dir()))
        })
        .filter(|(name, _, _)| !name.starts_with('.'))
        .collect();
    entries.sort();
    entries
}

/// The start of a plain file, never a link, a pipe or a device: the file is opened with no wait
/// and no link followed, then checked, as `notice.rs` reads a project file.
fn start_of(path: &Path) -> Option<String> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let flags =
        rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOCTTY | rustix::fs::OFlags::NOFOLLOW;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(flags.bits().cast_signed())
        .open(path)
        .ok()?;
    if !file.metadata().is_ok_and(|meta| meta.is_file()) {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MOST_READ).read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

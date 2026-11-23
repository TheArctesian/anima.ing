//! The commands and the skills of Claude Code in a folder before any thread runs there (TH117):
//! read from the files of `.claude/commands` and `.claude/skills` of the folder and of the home
//! folder, as Claude Code reads them. Only the start of each file is read, a link or a file that
//! is not a plain file is never opened, a folder that is a link is never walked, and the walk
//! stops at a fixed depth and count of entries, so a hostile folder cannot make the runner read
//! much or wait.

use std::io::Read as _;
use std::path::Path;

use agent_threads::drive::{CommandRow, MOST_ROWS, claude_file};

/// The most bytes of one file that are read: the frontmatter and the first line.
const MOST_READ: u64 = 8 * 1024;
/// The most folders below `.claude/commands` that a name may have.
const MOST_DEPTH: usize = 3;
/// The most entries of folders that the walk of one root looks at, in all.
const MOST_SEEN: usize = 2000;

/// The commands of the folder, then those of the home folder that the folder does not have.
#[must_use]
pub fn commands(folder: &Path, home: Option<&Path>) -> Vec<CommandRow> {
    let mut rows: Vec<CommandRow> = Vec::new();
    for root in std::iter::once(folder).chain(home) {
        let claude = root.join(".claude");
        if !plain_folder(&claude) {
            continue;
        }
        let mut walk = Walk {
            found: Vec::new(),
            left: MOST_SEEN,
        };
        walk.commands(&claude.join("commands"), "", 0);
        walk.skills(&claude.join("skills"));
        for row in walk.found {
            if rows.len() < MOST_ROWS && !rows.iter().any(|each| each.name == row.name) {
                rows.push(row);
            }
        }
    }
    rows
}

/// A folder that is a folder itself, not a link to one.
fn plain_folder(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_dir())
}

/// One walk of the `.claude` folder of a root: what it found and how many entries it may still
/// look at.
struct Walk {
    found: Vec<CommandRow>,
    left: usize,
}

impl Walk {
    /// The `.md` files below `folder`, named `prefix:name`.
    fn commands(&mut self, folder: &Path, prefix: &str, depth: usize) {
        for (name, path, is_dir) in self.entries(folder) {
            if self.found.len() >= MOST_ROWS {
                return;
            }
            let named = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}:{name}")
            };
            if is_dir && depth < MOST_DEPTH {
                self.commands(&path, &named, depth + 1);
            } else if let Some(command) = named.strip_suffix(".md").filter(|_| !is_dir) {
                self.found
                    .extend(start_of(&path).and_then(|text| claude_file(command, &text)));
            }
        }
    }

    /// Each `<name>/SKILL.md` of `folder`.
    fn skills(&mut self, folder: &Path) {
        for (name, path, is_dir) in self.entries(folder) {
            if self.found.len() >= MOST_ROWS {
                return;
            }
            if is_dir {
                let text = start_of(&path.join("SKILL.md"));
                self.found
                    .extend(text.and_then(|text| claude_file(&name, &text)));
            }
        }
    }

    /// The plain files and folders of `folder` that is not a link, by name, with no link and no
    /// hidden name; in the order of their names. Each entry read counts against the walk.
    fn entries(&mut self, folder: &Path) -> Vec<(String, std::path::PathBuf, bool)> {
        if self.left == 0 || !plain_folder(folder) {
            return Vec::new();
        }
        let Ok(read) = std::fs::read_dir(folder) else {
            return Vec::new();
        };
        let mut entries = Vec::new();
        for entry in read {
            if self.left == 0 {
                break;
            }
            self.left -= 1;
            let Ok(entry) = entry else { continue };
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if (kind.is_dir() || kind.is_file()) && !name.starts_with('.') {
                entries.push((name, entry.path(), kind.is_dir()));
            }
        }
        entries.sort();
        entries
    }
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

//! The files of the working folder of a thread, as the `@` list of the composer shows them
//! (TH108; report 03, "What @ lists"): which paths the list may name, and their order for what
//! the person typed.
//!
//! Pure: the runner walks the folder (I/O) and gives each path here. A path is relative to the
//! folder, with `/` between its parts. The files are never opened.

mod ignore;
mod secret;
#[cfg(test)]
mod tests;

pub use ignore::Ignored;
pub use secret::{is_secret, secret_paths};

use serde::{Deserialize, Serialize};

/// The most paths of one answer.
pub const MOST_FOUND: usize = 200;

/// The most bytes of what the person typed after `@`.
pub const MAX_QUERY: usize = 256;

/// Folders of tools, of dependencies and of build output: the list does not go into them.
const SKIPPED_FOLDERS: [&str; 16] = [
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
    ".svelte-kit",
    ".turbo",
    ".cache",
    "dist",
    "DerivedData",
    "Pods",
    ".gradle",
];

/// A file or a folder of the working folder of a thread.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundPath {
    /// From the working folder, with `/` between the parts.
    pub path: String,
    pub folder: bool,
}

/// Whether the list leaves this path out: a secret by its name or a folder on the way to it
/// (JOB-018), and, for a folder, one of tools or of build output, which it also does not enter.
#[must_use]
pub fn left_out(path: &str, folder: bool) -> bool {
    let name = path.rsplit('/').next().unwrap_or_default();
    if folder {
        SKIPPED_FOLDERS.contains(&name) || name.eq_ignore_ascii_case("secrets") || is_secret(path)
    } else {
        is_secret(path)
    }
}

/// The paths that match `query`, the best first, at most `most` of them, and whether more
/// matched. An empty query gives the top of the folder: its folders, then its files, by name.
#[must_use]
pub fn ranked(found: &[FoundPath], query: &str, most: usize) -> (Vec<FoundPath>, bool) {
    let asked = query.trim().to_lowercase();
    let mut scored: Vec<(u8, usize, &FoundPath)> = if asked.is_empty() {
        found
            .iter()
            .filter(|entry| !entry.path.contains('/'))
            .map(|entry| (u8::from(!entry.folder), 0, entry))
            .collect()
    } else {
        found
            .iter()
            .filter_map(|entry| {
                let score = score(&entry.path.to_lowercase(), &asked)?;
                Some((score, entry.path.matches('/').count(), entry))
            })
            .collect()
    };
    scored.sort_by(|a, b| (a.0, a.1, &a.2.path).cmp(&(b.0, b.1, &b.2.path)));
    let more = scored.len() > most;
    let best = scored
        .into_iter()
        .take(most)
        .map(|(_, _, entry)| entry.clone())
        .collect();
    (best, more)
}

/// How well `path` matches `asked` (both lower case): 0 is the best; `None` when it does not.
fn score(path: &str, asked: &str) -> Option<u8> {
    let name = path.rsplit('/').next().unwrap_or_default();
    if name == asked {
        Some(0)
    } else if name.starts_with(asked) {
        Some(1)
    } else if at_a_boundary(name, asked) {
        Some(2)
    } else if name.contains(asked) {
        Some(3)
    } else if path.contains(asked) {
        Some(4)
    } else {
        in_order(path, asked).then_some(5)
    }
}

/// `asked` starts a word of `name`: after `-`, `_`, `.` or a space.
fn at_a_boundary(name: &str, asked: &str) -> bool {
    name.match_indices(asked).any(|(at, _)| {
        name[..at]
            .chars()
            .next_back()
            .is_some_and(|before| matches!(before, '-' | '_' | '.' | ' '))
    })
}

/// Each character of `asked` is in `path`, in the same order.
fn in_order(path: &str, asked: &str) -> bool {
    let mut rest = path.chars();
    asked.chars().all(|wanted| rest.any(|had| had == wanted))
}

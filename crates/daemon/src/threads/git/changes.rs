//! The changed files between two commits (TH43): the files, their counts and the patch of each,
//! in the form the diff panel of the page reads.
//!
//! [`read`] is pure; [`between`] runs `git diff` with no external diff and no text conversion.

use std::collections::HashMap;

use agent_threads::git::{ChangeKind, ChangedFile, Changes};

use super::place::Worktree;
use super::run::{Git, GitError, Run};

/// The changes from the commit `from` to the commit `to`.
///
/// # Errors
/// A [`GitError`] when `git diff` fails. Patches over the limit of output are left out and
/// [`Changes::truncated`] says so.
pub async fn between(
    git: &Git,
    worktree: &Worktree,
    range: (&str, &str),
    ignore_whitespace: bool,
) -> Result<Changes, GitError> {
    let diff = |form: &[&str]| {
        let mut args = vec![
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--find-renames",
            "--submodule=short",
            // Between two commits git looks into no nested repository, so one that is new
            // in a snapshot is shown and not hidden.
            "--ignore-submodules=none",
            "--src-prefix=a/",
            "--dst-prefix=b/",
        ];
        if ignore_whitespace {
            args.push("--ignore-all-space");
        }
        args.extend_from_slice(form);
        args.extend([range.0, range.1, "--"]);
        git.output(Run::new(worktree.folder(), args))
    };
    let names = diff(&["--name-status", "-z"]).await?;
    let counts = diff(&["--numstat", "-z"]).await?;
    let patch = match diff(&["--patch"]).await {
        Ok(patch) => Some(String::from_utf8_lossy(&patch).into_owned()),
        Err(GitError::TooLarge) => None,
        Err(other) => return Err(other),
    };
    Ok(read(&names, &counts, patch.as_deref()))
}

/// Reads the three outputs of `git diff` (`--name-status -z`, `--numstat -z`, `--patch`) as one
/// list of files. `whole` (the patch of all files) is `None` when it was over the limit.
#[must_use]
pub fn read(names: &[u8], counts: &[u8], whole: Option<&str>) -> Changes {
    let counts = counted(counts);
    let mut by_file = whole.map_or_else(HashMap::new, by_path);
    let mut files = Vec::new();
    let mut fields = names
        .split(|byte| *byte == 0)
        .map(|field| String::from_utf8_lossy(field).into_owned());
    while let Some(status) = fields.next().filter(|status| !status.is_empty()) {
        let kind = match status.chars().next() {
            Some('A') => ChangeKind::Added,
            Some('D') => ChangeKind::Deleted,
            Some('R' | 'C') => ChangeKind::Renamed,
            _ => ChangeKind::Modified,
        };
        let old_path = (kind == ChangeKind::Renamed)
            .then(|| fields.next())
            .flatten();
        let Some(path) = fields.next() else { break };
        let (additions, deletions, binary) = counts.get(&path).copied().unwrap_or_default();
        files.push(ChangedFile {
            patch: by_file.remove(&path),
            path,
            old_path,
            kind,
            additions,
            deletions,
            binary,
        });
    }
    Changes {
        additions: files.iter().map(|file| file.additions).sum(),
        deletions: files.iter().map(|file| file.deletions).sum(),
        files,
        truncated: whole.is_none(),
        snapshot: None,
    }
}

/// The counts of `--numstat -z` by the new path of each file; `-` (a binary file) counts 0.
fn counted(counts: &[u8]) -> HashMap<String, (u32, u32, bool)> {
    let mut found = HashMap::new();
    let mut fields = counts
        .split(|byte| *byte == 0)
        .map(|field| String::from_utf8_lossy(field).into_owned());
    while let Some(row) = fields.next().filter(|row| !row.is_empty()) {
        let mut cells = row.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(path)) = (cells.next(), cells.next(), cells.next())
        else {
            continue;
        };
        // A rename has an empty path here, then the old and the new path as fields of their own.
        let path = if path.is_empty() {
            fields.next();
            fields.next().unwrap_or_default()
        } else {
            path.to_owned()
        };
        let binary = added == "-";
        found.insert(
            path,
            (
                added.parse().unwrap_or(0),
                deleted.parse().unwrap_or(0),
                binary,
            ),
        );
    }
    found
}

/// The patch of each file by its path: the new path, or the old one for a deleted file.
fn by_path(patch: &str) -> HashMap<String, String> {
    let mut found = HashMap::new();
    let mut starts: Vec<usize> = patch
        .match_indices("diff --git ")
        .map(|(at, _)| at)
        .filter(|at| *at == 0 || patch.as_bytes()[at - 1] == b'\n')
        .collect();
    starts.push(patch.len());
    for pair in starts.windows(2) {
        let one = &patch[pair[0]..pair[1]];
        if let Some(path) = path_of(one) {
            found.insert(path, one.to_owned());
        }
    }
    found
}

/// The path a patch of one file is for, from its header lines.
fn path_of(one: &str) -> Option<String> {
    let mut old = None;
    for line in one.lines().skip(1) {
        if line.starts_with("@@") {
            break;
        }
        if let Some(path) = line.strip_prefix("+++ b/") {
            return Some(path.to_owned());
        }
        if let Some(path) = line.strip_prefix("rename to ") {
            return Some(path.to_owned());
        }
        if let Some(path) = line.strip_prefix("--- a/") {
            old = Some(path.to_owned());
        }
    }
    // A binary file or a change of mode has no `+++` line: the header names `a/x b/x`.
    old.or_else(|| {
        let header = one.lines().next()?.strip_prefix("diff --git a/")?;
        let half = header.len().checked_sub(3)? / 2;
        let (left, right) = (header.get(..half)?, header.get(half..)?);
        (right.strip_prefix(" b/") == Some(left)).then(|| left.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_outputs_of_git_diff_are_one_list_of_files() {
        let names =
            b"M\0src/a.rs\0A\0new file.txt\0D\0gone.txt\0R090\0old.rs\0moved.rs\0M\0logo.png\0";
        let counts =
            b"2\t1\tsrc/a.rs\x003\t0\tnew file.txt\x000\t4\tgone.txt\x001\t1\t\0old.rs\0moved.rs\0-\t-\tlogo.png\0";
        let patch = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1,2 @@\n-x\n+y\n+z\n\
                     diff --git a/new file.txt b/new file.txt\nnew file mode 100644\n--- /dev/null\n+++ b/new file.txt\n@@ -0,0 +1,3 @@\n+1\n+2\n+3\n\
                     diff --git a/gone.txt b/gone.txt\ndeleted file mode 100644\n--- a/gone.txt\n+++ /dev/null\n@@ -1,4 +0,0 @@\n-1\n\
                     diff --git a/old.rs b/moved.rs\nsimilarity index 90%\nrename from old.rs\nrename to moved.rs\n\
                     diff --git a/logo.png b/logo.png\nBinary files a/logo.png and b/logo.png differ\n";
        let changes = read(names, counts, Some(patch));
        let seen: Vec<_> = changes
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind,
                    file.additions,
                    file.deletions,
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                ("src/a.rs", ChangeKind::Modified, 2, 1),
                ("new file.txt", ChangeKind::Added, 3, 0),
                ("gone.txt", ChangeKind::Deleted, 0, 4),
                ("moved.rs", ChangeKind::Renamed, 1, 1),
                ("logo.png", ChangeKind::Modified, 0, 0),
            ]
        );
        assert_eq!((changes.additions, changes.deletions), (6, 6));
        assert_eq!(changes.files[3].old_path.as_deref(), Some("old.rs"));
        assert!(changes.files[4].binary);
        assert!(changes.files.iter().all(|file| file.patch.is_some()));
        assert!(
            changes.files[0]
                .patch
                .as_deref()
                .is_some_and(|patch| patch.ends_with("+z\n"))
        );
        assert!(!changes.truncated);
    }

    #[test]
    fn patches_over_the_limit_leave_the_counts_whole() {
        let changes = read(b"M\0a\0", b"5\t2\ta\0", None);
        assert!(changes.truncated);
        assert_eq!((changes.additions, changes.deletions), (5, 2));
        assert_eq!(changes.files[0].patch, None);
    }
}

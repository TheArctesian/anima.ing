//! How the branch of a worktree stands (TH45): its name, its upstream, ahead and behind, the
//! default branch, and the files that changed since the last commit.
//!
//! [`read`] is pure; [`of`] runs the read-only commands of `git`.

use agent_threads::git::{ChangeKind, ChangedFile, Status};

use super::place::{self, Worktree};
use super::run::{Git, GitError, Run};

/// The status of the worktree.
///
/// # Errors
/// A [`GitError`] when a command of `git` fails.
pub async fn of(git: &Git, worktree: &Worktree) -> Result<Status, GitError> {
    let dir = worktree.folder();
    let porcelain = git
        .output(Run::new(
            dir,
            [
                "status",
                "--porcelain=2",
                "--branch",
                "-z",
                "--untracked-files=all",
            ],
        ))
        .await?;
    let mut status = read(&porcelain);
    let counts = match git
        .output(Run::new(
            dir,
            [
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--numstat",
                "-z",
                "HEAD",
                "--",
            ],
        ))
        .await
    {
        Ok(counts) => counts,
        // A repository with no commit yet has no `HEAD` to compare with.
        Err(GitError::Failed { .. }) => Vec::new(),
        Err(other) => return Err(other),
    };
    for (path, added, deleted) in numstat(&counts) {
        if let Some(file) = status.files.iter_mut().find(|file| file.path == path) {
            file.additions = added;
            file.deletions = deleted;
        }
    }
    status.additions = status.files.iter().map(|file| file.additions).sum();
    status.deletions = status.files.iter().map(|file| file.deletions).sum();
    let remote = remote(git, worktree).await?;
    status.has_remote = remote.is_some();
    status.default_branch = default_branch(git, worktree, remote.as_deref()).await?;
    status.is_default_branch = status.branch.is_some() && status.branch == status.default_branch;
    if let Some(default) = &status.default_branch {
        status.ahead_of_default = ahead_of(git, worktree, remote.as_deref(), default).await?;
    }
    Ok(status)
}

/// The remote a push goes to: the push remote of the person, else `origin`, else the first.
/// `None` when the repository has no remote or its name is not safe as an argument.
///
/// # Errors
/// A [`GitError`] when `git remote` fails.
pub async fn remote(git: &Git, worktree: &Worktree) -> Result<Option<String>, GitError> {
    let dir = worktree.folder();
    let listed = git.text(Run::new(dir, ["remote"])).await?;
    let names: Vec<&str> = listed.lines().collect();
    let set = match git
        .text(Run::new(dir, ["config", "--get", "remote.pushDefault"]))
        .await
    {
        Ok(set) => Some(set),
        Err(GitError::Failed { code: Some(1), .. }) => None,
        Err(other) => return Err(other),
    };
    let chosen = set
        .as_deref()
        .filter(|set| names.contains(set))
        .or_else(|| names.iter().copied().find(|name| *name == "origin"))
        .or_else(|| names.first().copied());
    Ok(chosen
        .filter(|name| place::branch_name(name).is_ok() && !name.contains('/'))
        .map(str::to_owned))
}

async fn exists(git: &Git, worktree: &Worktree, name: &str) -> Result<bool, GitError> {
    git.yes(Run::new(
        worktree.folder(),
        ["show-ref", "--verify", "--quiet", name],
    ))
    .await
}

/// The default branch: what the remote says its `HEAD` is, else `main`, else `master`.
async fn default_branch(
    git: &Git,
    worktree: &Worktree,
    remote: Option<&str>,
) -> Result<Option<String>, GitError> {
    if let Some(remote) = remote {
        let head = format!("refs/remotes/{remote}/HEAD");
        let run = Run::new(
            worktree.folder(),
            ["symbolic-ref", "--quiet", "--short", head.as_str()],
        );
        if let Ok(name) = git.text(run).await
            && let Some(branch) = name.strip_prefix(&format!("{remote}/"))
        {
            return Ok(Some(branch.to_owned()));
        }
    }
    for name in ["main", "master"] {
        if exists(git, worktree, &format!("refs/heads/{name}")).await? {
            return Ok(Some(name.to_owned()));
        }
    }
    Ok(None)
}

/// The count of the commits of `HEAD` that the default branch does not have.
async fn ahead_of(
    git: &Git,
    worktree: &Worktree,
    remote: Option<&str>,
    default: &str,
) -> Result<u32, GitError> {
    let tracked = remote.map(|remote| format!("refs/remotes/{remote}/{default}"));
    let local = format!("refs/heads/{default}");
    let base = match tracked {
        Some(tracked) if exists(git, worktree, &tracked).await? => tracked,
        _ if exists(git, worktree, &local).await? => local,
        _ => return Ok(0),
    };
    let range = format!("{base}..HEAD");
    let count = git
        .text(Run::new(
            worktree.folder(),
            ["rev-list", "--count", range.as_str(), "--"],
        ))
        .await;
    Ok(count.ok().and_then(|count| count.parse().ok()).unwrap_or(0))
}

/// The rows of `git diff --numstat -z` with no rename detection: path, added, deleted.
fn numstat(counts: &[u8]) -> Vec<(String, u32, u32)> {
    counts
        .split(|byte| *byte == 0)
        .filter_map(|row| {
            let row = String::from_utf8_lossy(row);
            let mut cells = row.splitn(3, '\t');
            let (added, deleted, path) = (cells.next()?, cells.next()?, cells.next()?);
            Some((
                path.to_owned(),
                added.parse().unwrap_or(0),
                deleted.parse().unwrap_or(0),
            ))
        })
        .collect()
}

/// Reads `git status --porcelain=2 --branch -z`: the branch lines and one file for each entry.
#[must_use]
pub fn read(porcelain: &[u8]) -> Status {
    let mut status = Status::default();
    let mut fields = porcelain
        .split(|byte| *byte == 0)
        .map(|field| String::from_utf8_lossy(field).into_owned());
    while let Some(field) = fields.next() {
        if let Some(head) = field.strip_prefix("# branch.head ") {
            status.branch = (head != "(detached)").then(|| head.to_owned());
        } else if let Some(upstream) = field.strip_prefix("# branch.upstream ") {
            status.upstream = Some(upstream.to_owned());
        } else if let Some(counts) = field.strip_prefix("# branch.ab ") {
            let mut both = counts.split(' ');
            let count = |cell: Option<&str>, sign: char| {
                cell.and_then(|cell| cell.strip_prefix(sign))
                    .and_then(|cell| cell.parse().ok())
                    .unwrap_or(0)
            };
            status.ahead = count(both.next(), '+');
            status.behind = count(both.next(), '-');
        } else if let Some(file) = entry(&field) {
            if file.kind == ChangeKind::Renamed {
                let old_path = fields.next();
                status.files.push(ChangedFile { old_path, ..file });
            } else {
                status.files.push(file);
            }
        }
    }
    status
}

/// One entry of the status as a file: `1` (changed), `2` (renamed), `u` (unmerged), `?` (new).
fn entry(field: &str) -> Option<ChangedFile> {
    let (mark, rest) = field.split_once(' ')?;
    let (skip, kind) = match mark {
        "?" => (0, ChangeKind::Added),
        "1" => (7, kind_of(rest)),
        "2" => (8, ChangeKind::Renamed),
        "u" => (9, ChangeKind::Modified),
        _ => return None,
    };
    let path = rest.splitn(skip + 1, ' ').nth(skip)?;
    Some(ChangedFile {
        path: path.to_owned(),
        kind,
        ..ChangedFile::default()
    })
}

/// The kind of a changed entry from its two letters: the index, then the worktree.
fn kind_of(rest: &str) -> ChangeKind {
    let letters = rest.split(' ').next().unwrap_or_default();
    if letters.contains('D') {
        ChangeKind::Deleted
    } else if letters.contains('A') {
        ChangeKind::Added
    } else {
        ChangeKind::Modified
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_branch_lines_and_each_kind_of_entry_are_read() {
        let porcelain = b"# branch.oid abc\0# branch.head anima/12345678\0# branch.upstream origin/anima/12345678\0\
            # branch.ab +2 -1\0\
            1 .M N... 100644 100644 100644 aaa bbb src/a file.rs\0\
            1 A. N... 000000 100644 100644 000 bbb added.rs\0\
            1 .D N... 100644 100644 000000 aaa aaa gone.rs\0\
            2 R. N... 100644 100644 100644 aaa bbb R100 new.rs\0old.rs\0\
            ? untracked.txt\0";
        let status = read(porcelain);
        assert_eq!(status.branch.as_deref(), Some("anima/12345678"));
        assert_eq!(status.upstream.as_deref(), Some("origin/anima/12345678"));
        assert_eq!((status.ahead, status.behind), (2, 1));
        let seen: Vec<_> = status
            .files
            .iter()
            .map(|file| (file.path.as_str(), file.kind))
            .collect();
        assert_eq!(
            seen,
            [
                ("src/a file.rs", ChangeKind::Modified),
                ("added.rs", ChangeKind::Added),
                ("gone.rs", ChangeKind::Deleted),
                ("new.rs", ChangeKind::Renamed),
                ("untracked.txt", ChangeKind::Added),
            ]
        );
        assert_eq!(status.files[3].old_path.as_deref(), Some("old.rs"));
    }

    #[test]
    fn a_detached_head_has_no_branch() {
        assert_eq!(
            read(b"# branch.oid abc\0# branch.head (detached)\0").branch,
            None
        );
    }
}

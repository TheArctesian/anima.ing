//! A build can hide a changed file from `git status` (`--skip-worktree`, `--assume-unchanged`, an
//! exclude rule), so each carried file is compared with the commit's blob directly before its use.

use std::path::Path;

use crate::builder::{
    job::Commit,
    plan::{Command, git},
};

pub fn on_disk(clone: &Path, path: &str) -> Command {
    git(clone, &["hash-object", "--no-filters", "--", path])
}

pub fn in_commit(clone: &Path, commit: &Commit, path: &str) -> Command {
    git(
        clone,
        &[
            "rev-parse",
            "--verify",
            &format!("{}:{path}", commit.as_str()),
        ],
    )
}

pub fn same_blob(on_disk: &[String], in_commit: &[String]) -> bool {
    object_name(on_disk).is_some_and(|name| object_name(in_commit) == Some(name))
}

fn object_name(lines: &[String]) -> Option<&str> {
    match lines {
        [line]
            if matches!(line.len(), 40 | 64)
                && line
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) =>
        {
            Some(line)
        }
        _ => None,
    }
}

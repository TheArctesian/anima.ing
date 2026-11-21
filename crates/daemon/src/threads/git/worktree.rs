//! The worktree of a thread (TH41): made from a repository of the person without a change of
//! its checkout, used again when it is there, and removed only after its work is saved to a
//! private ref (THR-097).
//!
//! I/O: runs `git` in the repository of the person only to add a branch and a worktree.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use agent_threads::git::GitRefusal;
use uuid::Uuid;

use super::checkpoint;
use super::place::{self, Home, Record, Worktree};
use super::run::{Git, Run};

/// A worktree that is ready for a thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Made {
    pub worktree: Worktree,
    /// The name of the repository, which is the name of the folder of the worktree.
    pub repository: String,
    pub branch: String,
    /// The worktree was there already; nothing was made.
    pub reused: bool,
}

/// What to make a worktree from.
#[derive(Clone, Copy, Debug)]
pub struct Plan<'a> {
    pub thread: Uuid,
    /// The repository of the person: the top folder of a checkout.
    pub project: &'a Path,
    /// The branch or commit to start from; `HEAD` of the project when `None`.
    pub base: Option<&'a str>,
}

/// The real top folder of the repository at `project`, which must not be in the home of the
/// worktrees.
async fn repository(git: &Git, home: &Home, project: &Path) -> Result<PathBuf, GitRefusal> {
    if !project.is_absolute() {
        return Err(GitRefusal::BadPath);
    }
    let real = std::fs::canonicalize(project).map_err(|_| GitRefusal::NotARepository)?;
    if std::fs::canonicalize(home.folder()).is_ok_and(|home| real.starts_with(home)) {
        return Err(GitRefusal::BadPath);
    }
    let top = git
        .text(Run::new(&real, ["rev-parse", "--show-toplevel"]))
        .await
        .map_err(|_| GitRefusal::NotARepository)?;
    let top = std::fs::canonicalize(top).map_err(|_| GitRefusal::NotARepository)?;
    if top == real {
        Ok(real)
    } else {
        Err(GitRefusal::NotARepository)
    }
}

/// Makes the worktree of `plan.thread` for the repository at `plan.project`, or gives the one
/// that is there. The checkout of the person is not switched and no file of it changes.
///
/// # Errors
/// A [`GitRefusal`] when the project is not a repository, the base is unknown, the branch name
/// is taken by something the runner did not make, or `git` fails.
pub async fn make(git: &Git, home: &Home, plan: Plan<'_>) -> Result<Made, GitRefusal> {
    let project = repository(git, home, plan.project).await?;
    // A checkout into the new worktree starts no filter program of the repository (THR-050).
    let git = &git.quiet_in(&project).await?;
    let name = project.file_name().map(|name| name.to_string_lossy());
    let repository = place::folder_name(name.as_deref().unwrap_or_default());
    let branch = place::thread_branch(plan.thread);
    let made = |worktree, reused| Made {
        worktree,
        repository: repository.clone(),
        branch: branch.clone(),
        reused,
    };
    if let Ok(worktree) = home.worktree(plan.thread, &repository) {
        // A worktree of the same name from a different repository is not this one.
        return if worktree.record().project == project {
            Ok(made(worktree, true))
        } else {
            Err(GitRefusal::BadPath)
        };
    }
    let folder = home.planned(plan.thread, &repository)?;
    if folder.symlink_metadata().is_ok() {
        // Something is at the place and it is not a worktree of the runner: it is not touched.
        return Err(GitRefusal::BadPath);
    }
    let start = start_of(git, &project, plan.base).await?;
    let branch_exists = has_ref(git, &project, &format!("refs/heads/{branch}")).await?;
    // A branch of this name that the runner did not keep for this thread is never taken over.
    if branch_exists && !has_ref(git, &project, &checkpoint::kept_ref(plan.thread)).await? {
        return Err(GitRefusal::BranchExists);
    }
    place::private_folder(&home.of_thread(plan.thread)).map_err(|_| GitRefusal::BadPath)?;
    if !branch_exists {
        let new = [
            "branch",
            "--no-track",
            "--",
            branch.as_str(),
            start.as_str(),
        ];
        git.output(Run::new(&project, new)).await?;
    }
    let add: [&OsStr; 5] = [
        "worktree".as_ref(),
        "add".as_ref(),
        "--".as_ref(),
        folder.as_os_str(),
        branch.as_ref(),
    ];
    if let Err(failed) = git.output(Run::new(&project, add)).await {
        if !branch_exists {
            // The branch was made a moment ago for this worktree and holds no work.
            let undo = ["branch", "-D", "--", branch.as_str()];
            let _ = git.output(Run::new(&project, undo)).await;
        }
        return Err(failed.into());
    }
    // The record is written now, before an agent runs in the worktree: each later run of `git`
    // is held to these places, whatever the files of the worktree say then.
    let record = record_of(git, &project, &folder, &branch).await?;
    home.keep(plan.thread, &repository, &record)?;
    let worktree = home.worktree(plan.thread, &repository)?;
    let git = &git.for_worktree(&worktree).await?;
    checkpoint::capture(git, &worktree, 0).await?;
    Ok(made(worktree, false))
}

/// Whether the repository at `project` has the ref `name`.
async fn has_ref(git: &Git, project: &Path, name: &str) -> Result<bool, GitRefusal> {
    let verify = ["show-ref", "--verify", "--quiet", name];
    Ok(git.yes(Run::new(project, verify)).await?)
}

/// The commit to start from: `base` when it names one, else `HEAD` of the project.
async fn start_of(git: &Git, project: &Path, base: Option<&str>) -> Result<String, GitRefusal> {
    let name = match base {
        Some(base) => place::branch_name(base)?,
        None => "HEAD",
    };
    let commit = format!("{name}^{{commit}}");
    git.text(Run::new(
        project,
        [
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            commit.as_str(),
        ],
    ))
    .await
    .map_err(|_| GitRefusal::UnknownBase)
}

/// The record of a worktree that `git worktree add` made a moment ago at `folder`.
async fn record_of(
    git: &Git,
    project: &Path,
    folder: &Path,
    branch: &str,
) -> Result<Record, GitRefusal> {
    let common = ["rev-parse", "--path-format=absolute", "--git-common-dir"];
    let common_dir = git.text(Run::new(project, common)).await?;
    let common_dir = std::fs::canonicalize(common_dir).map_err(|_| GitRefusal::NotARepository)?;
    let folder = std::fs::canonicalize(folder).map_err(|_| GitRefusal::NoWorktree)?;
    let pointer =
        std::fs::read_to_string(folder.join(".git")).map_err(|_| GitRefusal::NoWorktree)?;
    let git_dir = pointer
        .trim()
        .strip_prefix("gitdir: ")
        .ok_or(GitRefusal::NoWorktree)?;
    let git_dir = std::fs::canonicalize(git_dir).map_err(|_| GitRefusal::NoWorktree)?;
    Ok(Record {
        project: project.to_owned(),
        folder,
        git_dir,
        common_dir,
        branches: vec![branch.to_owned()],
    })
}

/// Saves the files of the worktree to the private ref of its thread, then removes the worktree
/// and then the checkpoints of the thread. The branch stays. Gives the name of the ref.
///
/// # Errors
/// A [`GitRefusal`] when the work could not be saved (nothing is removed then) or `git` fails.
pub async fn remove(git: &Git, home: &Home, worktree: &Worktree) -> Result<String, GitRefusal> {
    let held = &git.for_worktree(worktree).await?;
    let kept = checkpoint::kept_ref(worktree.thread());
    let commit = checkpoint::snapshot(held, worktree).await?;
    checkpoint::keep(held, worktree, &kept, &commit).await?;
    let turns = checkpoint::list(held, worktree).await?;
    // The removal runs from the repository of the record, not from what the worktree names.
    let mut git_dir = std::ffi::OsString::from("--git-dir=");
    git_dir.push(&worktree.record().common_dir);
    let remove: [&OsStr; 6] = [
        git_dir.as_os_str(),
        "worktree".as_ref(),
        "remove".as_ref(),
        "--force".as_ref(),
        "--".as_ref(),
        worktree.folder().as_os_str(),
    ];
    git.output(Run::new(home.folder(), remove)).await?;
    for checkpoint in turns {
        let name = checkpoint::turn_ref(worktree.thread(), checkpoint.turn);
        let forget: [&OsStr; 5] = [
            git_dir.as_os_str(),
            "update-ref".as_ref(),
            "--no-deref".as_ref(),
            "-d".as_ref(),
            name.as_ref(),
        ];
        git.output(Run::new(home.folder(), forget)).await?;
    }
    if let Some(repository) = worktree.folder().file_name().and_then(OsStr::to_str) {
        home.forget(worktree.thread(), repository);
    }
    // The folder of the thread goes when it is empty; one with a second repository stays.
    let _ = std::fs::remove_dir(home.of_thread(worktree.thread()));
    Ok(kept)
}

//! A commit that the person asked for (TH45): all files or the chosen ones, as they are in the
//! snapshot that the person looked at, on the branch of the thread or on a new one.
//!
//! The commit is made in the Git folder of the runner with `git commit-tree`, from the tree of
//! the snapshot. Its objects are then copied to the repository of the person as one pack
//! file, and the branch is moved there as a file ([`refs::put`]). No `git` runs with the Git
//! folder of the person, so no hook and no setting of it applies, also not one that is written
//! while the commit runs. The hooks of a commit run only for a thread with full access, and
//! then with the rights of that thread ([`Hooks`]).
//!
//! I/O: `git commit-tree`, `git pack-objects`, a private index for chosen files, the ref file.

use std::time::Duration;

use agent_threads::git::GitRefusal;

use super::checkpoint;
use super::place::{self, Worktree, own};
use super::project::refs::{self, Head, Was};
use super::run::hooks::Hooks;
use super::run::settings::Person;
use super::run::{Git, GitError, Run};

/// The most bytes of a commit message.
const MOST: usize = 64 * 1024;

/// The time a commit gets: the program that signs can wait for the person.
const TIME: Duration = Duration::from_secs(600);

/// What the person asked to commit.
#[derive(Clone, Copy, Debug)]
pub struct Asked<'a> {
    pub message: &'a str,
    /// The snapshot of the files that the person looked at: what is committed is from it.
    pub reviewed: &'a str,
    /// `None` is each changed file.
    pub files: Option<&'a [String]>,
    pub new_branch: Option<&'a str>,
    /// Who the commit is from.
    pub who: &'a Person,
    /// The hooks of the repository, for a thread that may run them.
    pub hooks: Option<Hooks<'a>>,
}

/// Commits and gives the ID of the commit with the branch it is on. The commit holds the files
/// as they are in the snapshot `asked.reviewed`, never an edit that came after it, and has one
/// parent: the branch as it was.
///
/// # Errors
/// A [`GitRefusal`] for an empty message, a path that leaves the worktree, a branch name that
/// is taken or not allowed, a branch that the runner did not make, a snapshot that is not the
/// one the person last looked at, nothing to commit, or a failure of `git`. `own_it` records a
/// branch that this commit made.
pub async fn make(
    git: &Git,
    worktree: &Worktree,
    asked: Asked<'_>,
    own_it: &(dyn Fn(&str) -> Result<(), GitRefusal> + Sync),
) -> Result<(String, Option<String>), GitRefusal> {
    let message = asked.message.trim();
    if message.is_empty() || message.len() > MOST {
        return Err(GitRefusal::EmptyMessage);
    }
    // Each check comes before the first change.
    let files = asked
        .files
        .map(|files| files.iter().map(|path| place::inside(path)).collect())
        .transpose()?;
    let layout = worktree.layout();
    let head = own::tip(git, worktree).await?;
    let branch = branch_for(worktree, &head, asked.new_branch)?;
    let tip = head.commit.as_deref();
    let reviewed = reviewed(git, worktree, asked.reviewed, tip).await?;
    let tree = tree_of(git, worktree, (reviewed, tip), files).await?;
    if let Some(tip) = tip
        && commit_of(git, worktree, &format!("{tip}^{{tree}}"))
            .await
            .as_deref()
            == Some(&tree)
    {
        return Err(GitRefusal::NothingToCommit);
    }
    let asked = Asked { message, ..asked };
    let commit = checked(git, worktree, (&tree, tip), asked).await?;
    // The branch moves only from where it was: a new branch must not exist, the branch of the
    // thread must still be at the parent of the commit.
    let was = match (asked.new_branch, tip) {
        (None, Some(tip)) => Was::At(tip),
        _ => Was::Absent,
    };
    let name = format!("refs/heads/{branch}");
    refs::put(&layout.common_dir, &name, &commit, was)?;
    if asked.new_branch.is_some() {
        own_it(&branch)?;
        refs::point_head(&layout, &branch)?;
    }
    // The index of the worktree follows the branch; its files stay as they are.
    own::follow(git, worktree, &commit).await?;
    if let Some(hooks) = asked.hooks {
        hooks.after_commit().await;
    }
    Ok((commit, Some(branch)))
}

/// Writes the commit of `tree` on `tip` and gives its objects to the repository of the
/// person. For a thread that runs the hooks of the repository, they see the commit before a
/// branch names it, can refuse it, and can change its message.
async fn checked(
    git: &Git,
    worktree: &Worktree,
    (tree, tip): (&str, Option<&str>),
    asked: Asked<'_>,
) -> Result<String, GitRefusal> {
    let commit = written(git, worktree, (tree, tip), (asked.message, asked.who)).await?;
    // The repository of the person gets the objects first, then the branch that names them.
    own::give(git, worktree, &commit, tip).await?;
    let Some(hooks) = asked.hooks else {
        return Ok(commit);
    };
    let left = hooks.before_commit(git, &commit, asked.message).await?;
    if left.is_empty() {
        return Err(GitRefusal::EmptyMessage);
    }
    if left == asked.message {
        return Ok(commit);
    }
    let commit = written(git, worktree, (tree, tip), (&left, asked.who)).await?;
    own::give(git, worktree, &commit, tip).await?;
    Ok(commit)
}

/// Writes the commit of `tree` on `tip` in the Git folder of the runner: from the person, and
/// signed when they sign their commits.
async fn written(
    git: &Git,
    worktree: &Worktree,
    (tree, tip): (&str, Option<&str>),
    (message, who): (&str, &Person),
) -> Result<String, GitError> {
    let mut write = Vec::new();
    for (name, value) in &who.signing {
        write.extend(["-c".to_owned(), format!("{name}={value}")]);
    }
    write.extend(["commit-tree", tree, "-m", message].map(str::to_owned));
    if let Some(tip) = tip {
        write.extend(["-p".to_owned(), tip.to_owned()]);
    }
    if git.signs() {
        write.push("-S".to_owned());
    }
    let mut write = Run::new(worktree.folder(), write)
        .for_the_person()
        .with_time(TIME);
    if let Some(name) = &who.name {
        write = write
            .with_env("GIT_AUTHOR_NAME", name)
            .with_env("GIT_COMMITTER_NAME", name);
    }
    if let Some(email) = &who.email {
        write = write
            .with_env("GIT_AUTHOR_EMAIL", email)
            .with_env("GIT_COMMITTER_EMAIL", email);
    }
    git.text(write).await
}

/// The branch the commit goes to: a new one that does not exist yet, or the one the worktree
/// is on when the runner made it for this thread (P12).
fn branch_for(
    worktree: &Worktree,
    head: &Head,
    new_branch: Option<&str>,
) -> Result<String, GitRefusal> {
    let Some(name) = new_branch else {
        return head
            .branch
            .clone()
            .filter(|on| worktree.owns(on))
            .ok_or(GitRefusal::NotOurBranch);
    };
    let name = place::branch_name(name)?;
    let common = &worktree.record().common_dir;
    if refs::resolve(common, &format!("refs/heads/{name}"))?.is_some() {
        return Err(GitRefusal::BranchExists);
    }
    Ok(name.to_owned())
}

/// The ID that `name` resolves to in the Git folder of the runner; `None` when it names
/// nothing.
async fn commit_of(git: &Git, worktree: &Worktree, name: &str) -> Option<String> {
    let verify = ["rev-parse", "--verify", "--quiet", "--end-of-options", name];
    git.text(Run::new(worktree.folder(), verify)).await.ok()
}

/// Checks that `id` is the snapshot that the person last looked at, of the branch as it is
/// now: the ID under the ref that the runner wrote for it, with the last commit of the branch
/// as its parent (or none, in a repository with no commit).
async fn reviewed<'a>(
    git: &Git,
    worktree: &Worktree,
    id: &'a str,
    tip: Option<&str>,
) -> Result<&'a str, GitRefusal> {
    let id = refs::id(id).ok_or(GitRefusal::NotReviewed)?;
    let kept = checkpoint::reviewed_ref(worktree.thread());
    let shown = commit_of(git, worktree, &format!("{kept}^{{commit}}")).await;
    let parent = commit_of(git, worktree, &format!("{id}^1^{{commit}}")).await;
    if shown.as_deref() == Some(id) && parent.as_deref() == tip {
        Ok(id)
    } else {
        Err(GitRefusal::NotReviewed)
    }
}

/// The tree to commit: the one of the snapshot, or the last commit with only the chosen files
/// as the snapshot has them, which passed [`place::inside`]. It is built in a private index:
/// no file and no index of the worktree is read or changed.
async fn tree_of<'a>(
    git: &Git,
    worktree: &'a Worktree,
    (reviewed, tip): (&str, Option<&str>),
    files: Option<Vec<&str>>,
) -> Result<String, GitRefusal> {
    let dir = worktree.folder();
    let Some(files) = files else {
        let whole = format!("{reviewed}^{{tree}}");
        return commit_of(git, worktree, &whole)
            .await
            .ok_or(GitRefusal::NotReviewed);
    };
    if files.is_empty() {
        return Err(GitRefusal::NothingToCommit);
    }
    let scratch = tempfile::tempdir().map_err(|_| GitRefusal::BadPath)?;
    let index = scratch.path().join("index");
    let private = |run: Run<'a>| run.with_env("GIT_INDEX_FILE", index.as_os_str());
    if let Some(tip) = tip {
        git.output(private(Run::new(dir, ["read-tree", tip])))
            .await?;
    }
    let mut paths = Vec::new();
    for path in files {
        paths.extend_from_slice(path.as_bytes());
        paths.push(0);
    }
    let source = format!("--source={reviewed}");
    let restore = [
        "--literal-pathspecs",
        "restore",
        source.as_str(),
        "--staged",
        "--pathspec-from-file=-",
        "--pathspec-file-nul",
    ];
    git.output(private(Run::new(dir, restore).with_input(paths)))
        .await?;
    Ok(git.text(private(Run::new(dir, ["write-tree"]))).await?)
}

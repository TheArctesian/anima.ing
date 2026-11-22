//! The revert of the files of a worktree to a checkpoint (TH44, the file part of "Edit from
//! here"). It takes a [`Worktree`], so it cannot run in the checkout of the person (P12).
//!
//! I/O: `git` checks the files out into a folder of the runner; the runner copies them into
//! the worktree with no link followed.

use agent_threads::git::GitRefusal;

use super::checkpoint;
use super::place::{self, Worktree, own};
use super::run::{Git, GitError, Run};
use super::worktree::files;

/// Puts the files back as they were at the checkpoint of `turn`. The files as they were before
/// are kept under a ref first, whose name is the answer, so that the revert can be undone
/// (THR-099). The checkpoints after `turn` are deleted. The branch does not move.
///
/// # Errors
/// [`GitRefusal::UnknownCheckpoint`] when the turn has none; a refusal with the words of `git`
/// when a step fails, after the files were put back as they were before.
pub async fn to_turn(git: &Git, worktree: &Worktree, turn: u32) -> Result<String, GitRefusal> {
    let target = checkpoint::list(git, worktree)
        .await?
        .into_iter()
        .find(|checkpoint| checkpoint.turn == turn)
        .ok_or(GitRefusal::UnknownCheckpoint)?;
    let undo = checkpoint::before_revert_ref(worktree.thread());
    // The snapshot is the files of now: what differs from the checkpoint is put back.
    let before = checkpoint::snapshot(git, worktree).await?;
    checkpoint::keep(git, worktree, &undo, &before).await?;
    if let Err(failed) = files_as_at(git, worktree, (&before, &target.commit)).await {
        // The files go back to the snapshot of a moment ago; the first failure is the answer.
        let _ = files_as_at(git, worktree, (&target.commit, &before)).await;
        return Err(failed.into());
    }
    checkpoint::forget(git, worktree, |other| other > turn).await?;
    // Nothing that was staged before the revert stays staged for the files of now.
    if let Some(tip) = own::tip(git, worktree).await?.commit {
        own::follow(git, worktree, &tip).await?;
    }
    Ok(undo)
}

/// Makes the files of the worktree equal to the tree of `to`, when they are as the tree of
/// `from` now: each file that differs is checked out by `git` into a folder of the runner and
/// copied into the worktree with no link followed ([`files`]); each file that `to` does not
/// have is removed. A file that git ignores is in neither tree and stays.
async fn files_as_at(
    git: &Git,
    worktree: &Worktree,
    (from, to): (&str, &str),
) -> Result<(), GitError> {
    let dir = worktree.folder();
    let differ = [
        "diff",
        "--no-renames",
        "--raw",
        "-z",
        "--no-abbrev",
        from,
        to,
        "--",
    ];
    let differ = git.output(Run::new(dir, differ)).await?;
    let (written, removed) = read(&differ);
    let restore = worktree.record().own.join("restore");
    let _ = std::fs::remove_dir_all(&restore);
    place::private_folder(&restore).map_err(GitError::Files)?;
    let index = restore.join("index");
    let files_of = restore.join("files");
    let mut prefix = files_of.clone().into_os_string();
    prefix.push("/");
    let tree = Run::new(dir, ["read-tree", to]).with_env("GIT_INDEX_FILE", &index);
    git.output(tree).await?;
    let mut names = Vec::new();
    for name in &written {
        names.extend_from_slice(name);
        names.push(0);
    }
    let mut prefixed = std::ffi::OsString::from("--prefix=");
    prefixed.push(&prefix);
    let out: [&std::ffi::OsStr; 4] = [
        "checkout-index".as_ref(),
        "-z".as_ref(),
        "--stdin".as_ref(),
        prefixed.as_ref(),
    ];
    let out = Run::new(dir, out).with_env("GIT_INDEX_FILE", &index);
    git.output(out.with_input(names)).await?;
    let root = files::Root::open(dir).map_err(GitError::Files)?;
    let safe = |name: &Vec<u8>| Some(files::path(name)).filter(|path| files::inside(path));
    // Removals first: a file can be where a folder comes, and a folder where a file comes.
    for path in removed.iter().filter_map(safe) {
        root.remove(&path).map_err(GitError::Files)?;
    }
    for path in written.iter().filter_map(safe) {
        root.put(&path, &files_of).map_err(GitError::Files)?;
    }
    let _ = std::fs::remove_dir_all(&restore);
    Ok(())
}

/// The mode of an entry that is a nested repository: its files are not touched.
const NESTED: &[u8] = b"160000";

/// Reads `git diff --raw -z`: the paths to write and the paths to remove.
fn read(raw: &[u8]) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let (mut written, mut removed) = (Vec::new(), Vec::new());
    let mut fields = raw.split(|byte| *byte == 0);
    while let (Some(about), Some(path)) = (fields.next(), fields.next()) {
        let mut cells = about.split(|byte| *byte == b' ');
        let (before, after) = (
            cells.next().unwrap_or_default(),
            cells.next().unwrap_or_default(),
        );
        let gone = cells.nth(2) == Some(b"D");
        if before.ends_with(NESTED) || after == NESTED {
            continue;
        }
        if gone {
            removed.push(path.to_vec());
        } else {
            written.push(path.to_vec());
        }
    }
    (written, removed)
}

#[cfg(test)]
mod tests {
    use super::read;

    #[test]
    fn the_raw_difference_gives_the_files_to_write_and_to_remove() {
        let raw = b":100644 100644 aaa bbb M\0a.txt\0:000000 120000 000 ccc A\0d/link\0\
            :100644 000000 aaa 000 D\0gone.txt\0:160000 160000 aaa bbb M\0sub\0";
        let (written, removed) = read(raw);
        assert_eq!(written, [b"a.txt".to_vec(), b"d/link".to_vec()]);
        assert_eq!(removed, [b"gone.txt".to_vec()]);
    }
}

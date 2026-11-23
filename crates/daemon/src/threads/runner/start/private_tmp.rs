//! The `TMPDIR` of each agent (THR-050): a folder of its thread in the folder of the runner,
//! never the `TMPDIR` of the login, so that the temporary files of an agent are not in a folder
//! that the other agents or the records of the Git engine share. `/tmp` stays a folder that each
//! sandboxed agent can write. The runner makes it before the thread has a seat in the
//! launcher, and removes it when the thread no longer runs.
//!
//! A thread that persists (R-432) keeps its `TMPDIR` for as long as it runs, also across a
//! restart of the runner: the folder is in the folder of the runner, and the runner removes only
//! the folders of threads that it knows as stopped, after it has read its records again. A
//! thread that stopped does not start again: a new start makes a new folder.
//!
//! I/O: makes, reads and removes folders.

use std::io;
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _};
use std::path::{Path, PathBuf};

use uuid::Uuid;

/// The folder of the `TMPDIR`s in the folder of the runner.
const FOLDER: &str = "tmp";

/// The names of the environment that name a temporary folder: each gets the private one.
pub const NAMES: [&str; 3] = ["TMPDIR", "TMP", "TEMP"];

/// Where the `TMPDIR` of `thread` is, made or not.
#[must_use]
pub fn folder(runner: &Path, thread: Uuid) -> PathBuf {
    runner.join(FOLDER).join(thread.to_string())
}

/// The `TMPDIR` of `thread`, made with mode 0700 when it is not there. A folder that is a link,
/// or that is not private to this user, is refused: an agent could have put it. Only the folder
/// of the runner is resolved; the two folders below it are checked, not followed, so a link put
/// there after the check cannot change the path that the agent gets.
///
/// # Errors
/// The I/O error, or `PermissionDenied` for a folder that is not private.
pub fn made(runner: &Path, thread: Uuid) -> io::Result<PathBuf> {
    let runner = std::fs::canonicalize(runner)?;
    private(&runner.join(FOLDER))?;
    let folder = folder(&runner, thread);
    private(&folder)?;
    Ok(folder)
}

/// Makes `folder` with mode 0700, or checks the one that is there.
fn private(folder: &Path) -> io::Result<()> {
    match std::fs::DirBuilder::new().mode(0o700).create(folder) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let meta = std::fs::symlink_metadata(folder)?;
            let mine = meta.uid() == rustix::process::geteuid().as_raw();
            // No right of the group or of others.
            #[expect(
                clippy::verbose_bit_mask,
                reason = "the mask names the rights of the group and of others"
            )]
            let only_mine = meta.mode() & 0o077 == 0;
            if meta.is_dir() && mine && only_mine {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "the temporary folder of a thread is not private",
                ))
            }
        }
        Err(error) => Err(error),
    }
}

/// Removes the `TMPDIR` of each thread that is not in `keep`, with what it holds. A link is
/// removed, not followed.
pub fn sweep(runner: &Path, keep: impl Fn(Uuid) -> bool) {
    let Ok(entries) = std::fs::read_dir(runner.join(FOLDER)) else {
        return;
    };
    for entry in entries.flatten() {
        let thread = entry
            .file_name()
            .to_str()
            .and_then(|name| Uuid::parse_str(name).ok());
        let Some(thread) = thread.filter(|thread| !keep(*thread)) else {
            continue;
        };
        let path = entry.path();
        let gone = match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(&path),
            Ok(_) => std::fs::remove_file(&path),
            Err(error) => Err(error),
        };
        if let Err(error) = gone {
            tracing::warn!(%error, %thread, "the temporary folder of a thread was not removed");
        }
    }
}

/// The environment of an agent with its private `TMPDIR` in place of each temporary folder of
/// the login.
#[must_use]
pub fn with_tmp(mut env: Vec<(String, String)>, tmp: &Path) -> Vec<(String, String)> {
    env.retain(|(name, _)| !NAMES.contains(&name.as_str()));
    let tmp = tmp.to_string_lossy().into_owned();
    env.extend(NAMES.map(|name| (name.to_owned(), tmp.clone())));
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thr_050_each_thread_gets_a_private_tmpdir_and_a_link_is_refused() {
        let runner = tempfile::tempdir().expect("a folder");
        let (one, two) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let first = made(runner.path(), one).expect("made");
        let real = std::fs::canonicalize(runner.path()).expect("the real path");
        assert_eq!(first, real.join(FOLDER).join(one.to_string()));
        assert_eq!(made(runner.path(), one).expect("again"), first);
        assert_ne!(made(runner.path(), two).expect("made"), first);
        let mode = std::fs::metadata(&first).expect("there").mode() & 0o777;
        assert_eq!(mode, 0o700);

        let elsewhere = tempfile::tempdir().expect("a folder");
        let three = Uuid::from_u128(3);
        let link = runner.path().join(FOLDER).join(three.to_string());
        std::os::unix::fs::symlink(elsewhere.path(), &link).expect("a link");
        assert!(
            made(runner.path(), three).is_err(),
            "a link is not followed"
        );

        sweep(runner.path(), |thread| thread == two);
        assert!(
            !first.exists(),
            "the folder of a thread that no longer runs goes"
        );
        assert!(runner.path().join(FOLDER).join(two.to_string()).is_dir());
        assert!(
            std::fs::symlink_metadata(&link).is_err(),
            "the link of a thread that no longer runs goes"
        );
        assert!(elsewhere.path().is_dir(), "a link goes, not its target");
    }

    #[test]
    fn thr_050_a_tmpdir_that_others_can_read_is_refused_and_its_content_goes_with_it() {
        use std::os::unix::fs::PermissionsExt as _;
        let runner = tempfile::tempdir().expect("a folder");
        let thread = Uuid::from_u128(4);
        let tmp = made(runner.path(), thread).expect("made");
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).expect("0755");
        assert!(
            made(runner.path(), thread).is_err(),
            "a folder that others can read"
        );

        let elsewhere = tempfile::tempdir().expect("a folder");
        let kept = elsewhere.path().join("kept");
        std::fs::write(&kept, "of a different folder").expect("a file");
        std::os::unix::fs::symlink(elsewhere.path(), tmp.join("link")).expect("a link inside");
        sweep(runner.path(), |_| false);
        assert!(std::fs::symlink_metadata(&tmp).is_err(), "the folder goes");
        assert!(kept.is_file(), "a link inside the folder is not followed");
    }

    #[test]
    fn thr_050_an_agent_gets_its_tmpdir_in_place_of_the_one_of_the_login() {
        let login = vec![
            ("TMPDIR".to_owned(), "/var/folders/x/T/".to_owned()),
            ("TMP".to_owned(), "/tmp".to_owned()),
            ("HOME".to_owned(), "/home/a".to_owned()),
        ];
        let env = with_tmp(login, Path::new("/run/t"));
        for name in NAMES {
            let values: Vec<_> = env.iter().filter(|(n, _)| n == name).collect();
            assert_eq!(values, [&(name.to_owned(), "/run/t".to_owned())]);
        }
        assert!(env.contains(&("HOME".to_owned(), "/home/a".to_owned())));
    }
}

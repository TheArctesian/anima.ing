//! The engine as the runner makes it, and the one shape of a read that its launcher starts.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_threads::git::GitRefusal;
use uuid::Uuid;

use super::super::shared::pack::is_read;
use super::{Places, Unwired, engine};
use crate::threads::git::run::GitError;
use crate::threads::git::run::hooks::{AsThread, Kind, ThreadRun};

const ID: &str = "0123456789abcdef0123456789abcdef01234567";

fn nothing() -> AsThread {
    AsThread::new(|_| Box::pin(async { Err(std::io::ErrorKind::Unsupported.into()) }))
}

/// A read as the engine makes it: the closed options, then `rev-list` of one commit.
fn listed(thread: Uuid, dir: &Path, git: &Path) -> ThreadRun {
    let quiet = [
        ("GIT_NO_LAZY_FETCH", "1"),
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_SSH_COMMAND", "false"),
    ];
    let closed = [
        "--no-optional-locks",
        "--no-pager",
        "-c",
        "protocol.allow=never",
        "-c",
        "core.sshCommand=false",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "core.alternateRefsCommand=",
        "--no-replace-objects",
        "-c",
        "core.compression=-1",
        "-c",
        "pack.compression=-1",
        "-c",
        "pack.window=10",
        "-c",
        "pack.depth=50",
    ];
    let list = [
        "rev-list",
        "--objects",
        "--quiet",
        "--end-of-options",
        ID,
        "--not",
    ];
    ThreadRun {
        kind: Kind::Read,
        thread,
        dir: dir.to_owned(),
        program: git.to_owned(),
        args: closed.into_iter().chain(list).map(OsString::from).collect(),
        env: quiet
            .into_iter()
            .map(|(name, value)| (name, OsString::from(value)))
            .collect(),
        input: None,
        within: Duration::from_secs(60),
    }
}

#[test]
fn only_the_one_shape_of_a_read_is_a_read() {
    let (thread, dir, git) = (
        Uuid::now_v7(),
        PathBuf::from("/w"),
        PathBuf::from("/usr/bin/git"),
    );
    let good = listed(thread, &dir, &git);
    assert!(is_read(&good, &git));
    let pack = |base: PathBuf| {
        let mut run = good.clone();
        let at = run.args.len() - 6;
        run.args.truncate(at);
        run.args
            .extend(["pack-objects", "--revs", "--quiet"].map(OsString::from));
        run.args.push(base.into_os_string());
        run.input = Some(PathBuf::from("/records/want"));
        run
    };
    let base = dir
        .join(format!(".anima-pack-{}", thread.as_simple()))
        .join("p");
    assert!(is_read(&pack(base), &git));
    let changed: [&dyn Fn(&mut ThreadRun); 9] = [
        &|run| run.kind = Kind::Hook,
        &|run| run.program = PathBuf::from("/tmp/git"),
        &|run| run.args.push(OsString::from("--exec=/bin/sh")),
        &|run| run.args[0] = OsString::from("-c"),
        &|run| run.env.push(("GIT_INDEX_FILE", OsString::from("/x"))),
        &|run| run.env[0].1 = OsString::from("0"),
        &|run| run.input = Some(PathBuf::from("/etc/passwd")),
        &|run| *run.args.last_mut().expect("an argument") = OsString::from("HEAD"),
        &|run| run.args.retain(|arg| arg != "--not"),
    ];
    for (at, change) in changed.iter().enumerate() {
        let mut run = good.clone();
        change(&mut run);
        assert!(!is_read(&run, &git), "change {at} passed");
    }
    // A pack of a different thread, or outside the worktree, is no read of this thread.
    assert!(!is_read(&pack(dir.join(".anima-pack-other/p")), &git));
    assert!(!is_read(&pack(PathBuf::from("/tmp/p")), &git));
}

#[test]
fn the_engine_is_made_with_its_records_and_its_home() {
    let keep = tempfile::tempdir().expect("a folder");
    let top = std::fs::canonicalize(keep.path()).expect("its real path");
    let records = top.join("records");
    let places = Places {
        anima: top.join("anima"),
        records: Some(records.clone()),
        ..Places::default()
    };
    let path = std::env::var_os("PATH").unwrap_or_default();
    let login = || [(OsString::from("PATH"), path.clone())];
    engine(login(), &places, nothing()).expect("an engine");
    let about = std::fs::metadata(&records).expect("the records");
    let mode = std::os::unix::fs::PermissionsExt::mode(&about.permissions());
    assert_eq!(mode & 0o077, 0, "the records are open to others");
    // A folder of worktrees in a repository is refused, and so is a login with no git.
    std::fs::create_dir_all(top.join("repo/.git")).expect("a repository");
    let chosen = Places {
        chosen: Some(top.join("repo/worktrees")),
        ..places.clone()
    };
    let refused = engine(login(), &chosen, nothing()).expect_err("a home in a repository");
    assert!(matches!(refused, Unwired::Home(_)), "{refused:?}");
    let no_git = engine(
        [(OsString::from("PATH"), OsString::from("/nowhere"))],
        &places,
        nothing(),
    );
    if let Err(Unwired::Git(GitError::NotFound)) = &no_git {
        assert_eq!(
            no_git.expect_err("no git").refusal(),
            GitRefusal::GitNotFound
        );
    }
    assert_eq!(Unwired::NotOnWindows.refusal(), GitRefusal::NotOnWindows);
}

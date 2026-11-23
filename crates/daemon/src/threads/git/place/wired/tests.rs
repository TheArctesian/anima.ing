//! The engine as the runner makes it, and the one shape of a read that its launcher starts.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_threads::git::GitRefusal;
use uuid::Uuid;

use super::super::records::BadRecords;
use super::super::shared::pack::{closed, is_read, list_args};
use super::{Places, Unwired, engine};
use crate::threads::git::run::GitError;
use crate::threads::git::run::hooks::{AsThread, Kind, ThreadRun};

const ID: &str = "0123456789abcdef0123456789abcdef01234567";

fn nothing() -> AsThread {
    AsThread::new(|_| Box::pin(async { Err(std::io::ErrorKind::Unsupported.into()) }))
}

/// A read as the engine makes it: the closed options, then `rev-list` of one commit with one
/// known commit.
fn listed(thread: Uuid, dir: &Path, git: &Path) -> ThreadRun {
    let quiet = [
        ("GIT_NO_LAZY_FETCH", "1"),
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_SSH_COMMAND", "false"),
    ];
    ThreadRun {
        kind: Kind::Read,
        thread,
        dir: dir.to_owned(),
        program: git.to_owned(),
        args: closed()
            .into_iter()
            .chain(list_args(&[ID], &[ID]))
            .collect(),
        env: quiet
            .into_iter()
            .map(|(name, value)| (name, OsString::from(value)))
            .collect(),
        input: None,
        within: Duration::from_secs(60),
    }
}

/// The pack read of `thread` in `dir` as the engine makes it, from a list read.
fn packed(good: &ThreadRun, records: &Path) -> ThreadRun {
    let mut run = good.clone();
    let ours = run
        .dir
        .join(format!(".anima-pack-{}", run.thread.as_simple()));
    run.args = closed()
        .into_iter()
        .chain(["pack-objects", "--revs", "--quiet"].map(OsString::from))
        .chain([ours.join("p").into_os_string()])
        .collect();
    run.env.push((
        "GIT_OBJECT_DIRECTORY",
        ours.join("objects").into_os_string(),
    ));
    run.env.push((
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        OsString::from("/p/.git/objects"),
    ));
    let own = records.join(run.thread.to_string()).join("engine.git");
    run.input = Some(own.join("want"));
    run
}

type Change = &'static dyn Fn(&mut ThreadRun);

#[test]
fn only_the_one_shape_of_a_read_is_a_read() {
    let (thread, dir) = (Uuid::now_v7(), PathBuf::from("/w"));
    let (git, records) = (PathBuf::from("/usr/bin/git"), PathBuf::from("/records"));
    let good = listed(thread, &dir, &git);
    let pack = packed(&good, &records);
    assert!(is_read(&good, &git, &records));
    assert!(is_read(&pack, &git, &records));
    let both: [Change; 8] = [
        &|run| run.kind = Kind::Hook,
        &|run| run.program = PathBuf::from("/tmp/git"),
        &|run| run.args.push(OsString::from("--exec=/bin/sh")),
        &|run| run.args[0] = OsString::from("-c"),
        &|run| run.env.push(("GIT_INDEX_FILE", OsString::from("/x"))),
        &|run| run.env[0].1 = OsString::from("0"),
        &|run| run.dir = PathBuf::from("-w"),
        &|run| run.dir = PathBuf::from("/w/../x"),
    ];
    let listing: [Change; 4] = [
        &|run| run.input = Some(PathBuf::from("/records/want")),
        &|run| *run.args.last_mut().expect("an argument") = OsString::from("^HEAD"),
        &|run| run.args.push(OsString::from("--not")),
        &|run| run.args.truncate(run.args.len() - 2),
    ];
    let packing: [Change; 7] = [
        &|run| run.input = Some(PathBuf::from("/etc/passwd")),
        &|run| run.input = Some(PathBuf::from("/records/other/engine.git/want")),
        &|run| run.input = None,
        &|run| run.env.truncate(3),
        &|run| run.env[3].1 = OsString::from("/elsewhere"),
        &|run| run.env[4].1 = OsString::from("/a:/b"),
        &|run| *run.args.last_mut().expect("an argument") = OsString::from("/tmp/p"),
    ];
    let cases = both
        .iter()
        .map(|change| (&good, change))
        .chain(both.iter().map(|change| (&pack, change)))
        .chain(listing.iter().map(|change| (&good, change)))
        .chain(packing.iter().map(|change| (&pack, change)));
    for (at, (run, change)) in cases.enumerate() {
        let mut run = (*run).clone();
        change(&mut run);
        assert!(!is_read(&run, &git, &records), "change {at} passed");
    }
}

#[test]
fn git_takes_the_list_read_as_the_engine_sends_it() {
    let keep = tempfile::tempdir().expect("a folder");
    let git = |args: &[&str]| {
        let done = std::process::Command::new("git")
            .args(["-c", "user.name=A", "-c", "user.email=a@example.invalid"])
            .args(args)
            .current_dir(keep.path())
            .output()
            .expect("git runs");
        assert!(
            done.status.success(),
            "{}",
            String::from_utf8_lossy(&done.stderr)
        );
        String::from_utf8_lossy(&done.stdout).trim().to_owned()
    };
    git(&["init", "--quiet"]);
    git(&["commit", "--quiet", "--allow-empty", "-m", "one"]);
    let first = git(&["rev-parse", "HEAD"]);
    git(&["commit", "--quiet", "--allow-empty", "-m", "two"]);
    let second = git(&["rev-parse", "HEAD"]);
    let args: Vec<String> = closed()
        .into_iter()
        .chain(list_args(&[&second], &[&first]))
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    git(&args.iter().map(String::as_str).collect::<Vec<_>>());
}

#[test]
fn the_engine_is_made_with_its_records_and_its_home() {
    let keep = tempfile::tempdir().expect("a folder");
    let top = std::fs::canonicalize(keep.path()).expect("its real path");
    let records = top.join("records");
    let places = Places {
        anima: top.join("anima"),
        records: Some(records.clone()),
        // The folder of this test may be in `/tmp`.
        private_tmp: true,
        ..Places::default()
    };
    let path = std::env::var_os("PATH").unwrap_or_default();
    let login = || [(OsString::from("PATH"), path.clone())];
    let wired = engine(login(), &places, nothing()).expect("an engine");
    // The launcher checks each read against the same `git` and the same records.
    assert!(wired.program().is_absolute());
    assert_eq!(wired.records_folder(), records.as_path());
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

#[test]
fn records_in_a_temporary_folder_are_refused_until_each_agent_has_its_own() {
    let keep = tempfile::tempdir_in("/tmp").expect("a folder in /tmp");
    let in_tmp = keep.path().join("records");
    // Not in `/tmp`, so that only the name of the login refuses the records below.
    let keep_home = tempfile::tempdir_in("/var/tmp").expect("a folder in /var/tmp");
    let home = std::fs::canonicalize(keep_home.path()).expect("its real path");
    let real_tmp = std::fs::canonicalize("/tmp").expect("/tmp");
    assert!(!home.starts_with(&real_tmp));
    let path = std::env::var_os("PATH").unwrap_or_default();
    let login_with = |name: &str, tmpdir: &Path| {
        [
            (OsString::from("PATH"), path.clone()),
            (OsString::from(name), tmpdir.as_os_str().to_owned()),
        ]
    };
    let login = |tmpdir: &Path| login_with("TMPDIR", tmpdir);
    let places = |records: &Path| Places {
        anima: home.as_path().join("anima"),
        records: Some(records.to_owned()),
        ..Places::default()
    };
    let refused = |made: Result<_, Unwired>| {
        let said = made.expect_err("records in a temporary folder");
        assert!(
            matches!(said, Unwired::Records(BadRecords::InTemp)),
            "{said:?}"
        );
        said.refusal()
    };
    let words = refused(engine(login(home.as_path()), &places(&in_tmp), nothing()));
    assert_eq!(
        words,
        GitRefusal::Failed {
            said: "the records of Anima's Git are in a temporary folder that an agent can write"
                .into()
        }
    );
    assert!(!in_tmp.exists(), "a refused place is never made");
    // As `TMPDIR=/var/tmp` on Linux: the records of the user are in the `TMPDIR` of the login.
    let shared = home.as_path().join("shared");
    std::fs::create_dir(&shared).expect("a folder");
    let in_tmpdir = shared.join("anima-501");
    for name in ["TMPDIR", "TMP", "TEMP", "XDG_RUNTIME_DIR"] {
        refused(engine(
            login_with(name, &shared),
            &places(&in_tmpdir),
            nothing(),
        ));
    }
    refused(engine(
        login(Path::new("tmp")),
        &places(&in_tmpdir),
        nothing(),
    ));
    // When the runner gives each agent its own `TMPDIR`, the root check of each thread holds.
    let private = Places {
        private_tmp: true,
        ..places(&in_tmpdir)
    };
    engine(login(&shared), &private, nothing()).expect("an engine");
}

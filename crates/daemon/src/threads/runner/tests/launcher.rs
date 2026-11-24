//! The launcher of the Git engine (THR-050): a program runs only for a thread in Full access,
//! in its folder, with the environment of the agents and the mark of the thread, and its
//! process group ends at its time limit.

use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_threads::drive::{Mode, Program};
use uuid::Uuid;

use crate::threads::git::run::hooks::{Ended, Kind, ThreadRun};
use crate::threads::runner::launcher::read::Confinement;
use crate::threads::runner::launcher::{Rights, Seat, SharedRights, start};

mod reads;

fn rights(mode: Mode, folder: &Path) -> (SharedRights, Uuid) {
    of(Program::ClaudeCode, mode, folder)
}

/// The rights of one thread of `program` in `mode` in `folder`, with `/usr/bin/git` as the git
/// of the engine.
fn of(program: Program, mode: Mode, folder: &Path) -> (SharedRights, Uuid) {
    let thread = Uuid::now_v7();
    let confinement = Confinement {
        mode,
        program,
        codex: None,
        kept: None,
    };
    let tmp = folder.join(".tmp");
    std::fs::create_dir_all(&tmp).expect("the TMPDIR of the thread");
    let seat = Seat::new(folder.to_owned(), confinement, tmp);
    let rights = Rights {
        env: vec![
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
            ("ANIMA_SECRET".to_owned(), "s3cret".to_owned()),
            ("GIT_INDEX_FILE".to_owned(), "/the/login/index".to_owned()),
        ],
        threads: [(thread, seat)].into(),
        git: Some(PathBuf::from("/usr/bin/git")),
        records: Some(folder.join(".records")),
        ..Rights::default()
    };
    (Arc::new(Mutex::new(rights)), thread)
}

/// `/bin/sh -c script` for `thread` in `dir`.
fn shell(thread: Uuid, dir: &Path, script: &str) -> ThreadRun {
    ThreadRun {
        kind: Kind::Hook,
        thread,
        dir: dir.to_owned(),
        program: PathBuf::from("/bin/sh"),
        args: vec![OsString::from("-c"), OsString::from(script)],
        env: Vec::new(),
        input: None,
        within: Duration::from_secs(20),
    }
}

#[tokio::test]
async fn thr_050_a_run_gets_the_environment_of_the_agents_its_mark_and_its_input() {
    let folder = tempfile::tempdir().expect("a folder");
    let (rights, thread) = rights(Mode::FullAccess, folder.path());
    let input = folder.path().join("input");
    std::fs::write(&input, "from the engine").expect("the input");
    let mut run = shell(
        thread,
        folder.path(),
        "env >&2; read line; echo \"read: $line\" >&2; exit 3",
    );
    run.env = vec![("GIT_INDEX_FILE", OsString::from("/the/engine/index"))];
    run.input = Some(input);
    let Ended { code, said } = start(&rights, run).await.expect("it ran");
    assert_eq!(code, Some(3));
    assert!(said.contains("read: from the engine"));
    assert!(said.contains(&format!("AGENT_THREAD={thread}")));
    assert!(said.contains("GIT_INDEX_FILE=/the/engine/index"));
    assert!(
        !said.contains("ANIMA_"),
        "no name of Anima reaches the program"
    );
    assert!(!said.contains("/the/login/index"));
}

#[tokio::test]
async fn thr_050_a_run_is_refused_outside_the_rights_of_its_thread() {
    let folder = tempfile::tempdir().expect("a folder");
    let elsewhere = tempfile::tempdir().expect("a second folder");
    let (full, thread) = rights(Mode::FullAccess, folder.path());
    for mode in [Mode::Ask, Mode::AcceptEdits] {
        let (confined, confined_thread) = rights(mode, folder.path());
        let refused = start(&confined, shell(confined_thread, folder.path(), "true")).await;
        assert_eq!(
            refused.map_err(|error| error.kind()),
            Err(ErrorKind::PermissionDenied)
        );
    }
    let unknown = shell(Uuid::now_v7(), folder.path(), "true");
    let outside = shell(thread, elsewhere.path(), "true");
    let mut relative = shell(thread, folder.path(), "true");
    relative.program = PathBuf::from("sh");
    let mut named = shell(thread, folder.path(), "true");
    named.env = vec![("GIT_DIR", OsString::from("/elsewhere"))];
    for run in [unknown, outside, relative, named] {
        let refused = start(&full, run).await;
        assert_eq!(
            refused.map_err(|error| error.kind()),
            Err(ErrorKind::PermissionDenied)
        );
    }
}

#[tokio::test]
async fn thr_050_a_run_that_takes_too_long_ends_with_each_process_of_its_group() {
    let folder = tempfile::tempdir().expect("a folder");
    let (rights, thread) = rights(Mode::FullAccess, folder.path());
    let mut run = shell(
        thread,
        folder.path(),
        "sleep 300 & echo $! > child.pid; wait",
    );
    run.within = Duration::from_millis(500);
    let ended = start(&rights, run).await;
    assert_eq!(
        ended.map_err(|error| error.kind()),
        Err(ErrorKind::TimedOut)
    );
    let pid: i32 = std::fs::read_to_string(folder.path().join("child.pid"))
        .expect("the child wrote its ID")
        .trim()
        .parse()
        .expect("an ID");
    let mut gone = false;
    for _ in 0..40 {
        let alive = std::process::Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .status()
            .is_ok_and(|status| status.success());
        if !alive {
            gone = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(gone, "the child of the run ended with it");
}

#[test]
fn thr_050_a_thread_in_a_folder_above_the_records_holds_them() {
    let home = tempfile::tempdir().expect("a folder");
    let records = home.path().join("anima/records");
    std::fs::create_dir_all(&records).expect("the records");
    let project = home.path().join("project");
    std::fs::create_dir_all(&project).expect("a project");
    let (in_project, _) = rights(Mode::Ask, &project);
    // `/tmp` is a root of each sandboxed thread: records there are held by each of them.
    let in_tmp = std::fs::canonicalize(&records)
        .is_ok_and(|real| std::fs::canonicalize("/tmp").is_ok_and(|tmp| real.starts_with(tmp)));
    assert_eq!(in_project.lock().expect("rights").hold(&records), in_tmp);
    let (in_home, _) = rights(Mode::Ask, home.path());
    assert!(in_home.lock().expect("rights").hold(&records));
}

#[test]
fn thr_050_a_sandboxed_thread_holds_records_in_its_tmpdir() {
    let folder = tempfile::tempdir().expect("a folder");
    let project = folder.path().join("project");
    std::fs::create_dir(&project).expect("a project");
    let (rights, _) = rights(Mode::Ask, &project);
    let records = project.join(".tmp/records");
    std::fs::create_dir(&records).expect("the records");
    // A folder of the thread that does not hold the records, and that resolves.
    std::fs::create_dir(folder.path().join("elsewhere")).expect("a folder");
    let tmp_only = Seat::new(
        folder.path().join("elsewhere"),
        rights
            .lock()
            .expect("rights")
            .threads
            .values()
            .next()
            .expect("a seat")
            .confinement
            .clone(),
        project.join(".tmp"),
    );
    let mut rights = rights.lock().expect("rights");
    let thread = *rights.threads.keys().next().expect("a thread");
    rights.threads.insert(thread, tmp_only);
    let seat = &rights.threads[&thread];
    assert!(
        seat.roots.contains(&project.join(".tmp")),
        "its TMPDIR is a root"
    );
    assert!(rights.hold(&records), "the agent can write its TMPDIR");
}

#[tokio::test]
async fn thr_050_a_run_ends_with_its_program_though_a_background_child_holds_its_error_stream() {
    let folder = tempfile::tempdir().expect("a folder");
    let (rights, thread) = rights(Mode::FullAccess, folder.path());
    let mut run = shell(
        thread,
        folder.path(),
        "echo begun >&2; sleep 30 & echo $! > child.pid; exit 0",
    );
    run.within = Duration::from_secs(10);
    let started = std::time::Instant::now();
    let Ended { code, said } = start(&rights, run).await.expect("it ended");
    assert_eq!(code, Some(0));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "git does not wait for it either"
    );
    assert!(
        said.contains("begun"),
        "what it said before its end is kept"
    );
    let pid =
        std::fs::read_to_string(folder.path().join("child.pid")).expect("the child wrote its ID");
    let alive = |signal: &str| {
        std::process::Command::new("/bin/kill")
            .args([signal, pid.trim()])
            .status()
            .is_ok_and(|status| status.success())
    };
    assert!(
        alive("-0"),
        "as with git, the background child keeps running"
    );
    assert!(alive("-9"), "the test ends its child");
}

#[test]
fn thr_050_a_running_full_access_thread_does_not_hold_the_engine() {
    let folder = tempfile::tempdir().expect("a folder");
    let records = folder.path().join(".records");
    std::fs::create_dir(&records).expect("the records");
    let (full, _) = rights(Mode::FullAccess, folder.path());
    let held = full.lock().expect("the rights").hold(&records);
    assert!(
        !held,
        "Full access has no sandbox: the ruling of 2026-11-23 leaves it out"
    );

    let (sandboxed, _) = rights(Mode::AcceptEdits, folder.path());
    let held = sandboxed.lock().expect("the rights").hold(&records);
    assert!(
        held,
        "a sandboxed thread whose folder holds the records holds the engine"
    );
}

/// A seat in `mode` in `folder` with the `TMPDIR` `tmp`, both made.
fn seat(mode: Mode, folder: &Path, tmp: &Path) -> Seat {
    std::fs::create_dir_all(folder).expect("the folder");
    std::fs::create_dir_all(tmp).expect("the TMPDIR");
    let confinement = Confinement {
        mode,
        program: Program::ClaudeCode,
        codex: None,
        kept: None,
    };
    Seat::new(folder.to_owned(), confinement, tmp.to_owned())
}

#[test]
fn thr_050_a_sandbox_root_shared_with_a_different_running_thread_is_refused() {
    let folder = tempfile::tempdir().expect("a folder");
    // A `TMPDIR` is a real path, as `private_tmp::made` gives it.
    let home = std::fs::canonicalize(folder.path()).expect("the real path");
    let at = |name: &str| home.join(name);
    let records = at("records");
    std::fs::create_dir(&records).expect("the records");
    let first = seat(Mode::Ask, &at("project"), &at("runner/tmp/1"));

    let same_folder = seat(Mode::AcceptEdits, &at("project"), &at("runner/tmp/2"));
    assert_eq!(
        same_folder.shared([&first], Some(&records)),
        None,
        "two threads in one folder are permitted"
    );

    // A thread whose folder holds the folder of the runner can write the `TMPDIR` of the new one.
    let above = seat(Mode::Ask, home.as_path(), &at("elsewhere/tmp"));
    let said = same_folder.shared([&above], None).expect("refused");
    assert!(said.contains("a different running thread"), "{said}");
    let full = seat(Mode::FullAccess, home.as_path(), &at("elsewhere/tmp"));
    assert_eq!(
        same_folder.shared([&full], None),
        None,
        "a thread in Full access is not checked"
    );

    let nested = seat(Mode::Ask, &at("project/sub"), &at("runner/tmp/3"));
    let said = nested.shared([&first], None).expect("refused");
    assert!(
        said.contains("a different running thread"),
        "a folder inside a folder: {said}"
    );
    // The new thread can write the `TMPDIR` of the running one.
    let holds_runner = seat(Mode::Ask, &at("runner"), &at("elsewhere/tmp/4"));
    let said = holds_runner.shared([&first], None).expect("refused");
    assert!(said.contains("a different running thread"), "{said}");

    let in_records = seat(Mode::Ask, &at("other"), &records.join("tmp"));
    let said = in_records.shared([], Some(&records)).expect("refused");
    assert!(said.contains("records of the Git engine"), "{said}");
    let unsandboxed = seat(Mode::FullAccess, &at("other"), &records.join("tmp"));
    assert_eq!(unsandboxed.shared([&first], Some(&records)), None);
}

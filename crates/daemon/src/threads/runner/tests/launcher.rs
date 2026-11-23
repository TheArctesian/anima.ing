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

use crate::threads::git::run::hooks::{Ended, ThreadRun};
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
    let seat = Seat {
        folder: folder.to_owned(),
        confinement,
    };
    let rights = Rights {
        env: vec![
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
            ("ANIMA_SECRET".to_owned(), "s3cret".to_owned()),
            ("GIT_INDEX_FILE".to_owned(), "/the/login/index".to_owned()),
        ],
        threads: [(thread, seat)].into(),
        git: Some(PathBuf::from("/usr/bin/git")),
        ..Rights::default()
    };
    (Arc::new(Mutex::new(rights)), thread)
}

/// `/bin/sh -c script` for `thread` in `dir`.
fn shell(thread: Uuid, dir: &Path, script: &str) -> ThreadRun {
    ThreadRun {
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
    let (in_project, _) = rights(Mode::FullAccess, &project);
    assert!(!in_project.lock().expect("rights").hold(&records));
    let (in_home, _) = rights(Mode::Ask, home.path());
    assert!(in_home.lock().expect("rights").hold(&records));
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

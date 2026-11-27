use std::{
    fs,
    os::unix::fs::{PermissionsExt as _, symlink},
    os::unix::net::UnixListener,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use super::*;
use crate::Command;

fn fixture() -> (tempfile::TempDir, Paths, UnixListener) {
    let directory = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let root = directory.path().canonicalize().unwrap();
    let socket = root.join("engine.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let paths = Paths {
        docker: executable("sh"),
        compose: executable("false"),
        configuration: root.display().to_string(),
        socket: socket.display().to_string(),
        root: root.display().to_string(),
    };
    (directory, paths, listener)
}

fn executable(name: &str) -> String {
    std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|directory| directory.join(name))
        .find(|path| path.is_absolute() && path.is_file())
        .unwrap()
        .display()
        .to_string()
}

fn command(paths: &Paths, script: &str, arguments: &[&str]) -> Command {
    let mut args = vec!["-c".to_owned(), script.to_owned(), "fixture".into()];
    args.extend(arguments.iter().map(|value| (*value).to_owned()));
    Command {
        program: paths.docker.clone(),
        arguments: args,
        environment: paths.environment(),
        folder: None,
    }
}

fn step(path: &Path, bytes: &[u8]) -> Step {
    Step::Write {
        path: path.display().to_string(),
        content: bytes.to_vec(),
    }
}

#[test]
fn failed_commands_preserve_the_primary_error_and_still_run_required_cleanup() {
    let (_directory, paths, _listener) = fixture();
    let root = Path::new(&paths.root);
    fs::create_dir(root.join("cleanup")).unwrap();
    let nested = root.join("cleanup/nested");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("delete"), b"old").unwrap();
    let mut executor = Executor::open(&paths).unwrap();
    assert!(Executor::open(&paths).is_err());
    let plan = Plan {
        steps: vec![
            Step::Attempt(command(&paths, "exit 7", &[])),
            step(&root.join("kept"), b"private"),
            Step::Run(command(&paths, "exit 9", &[])),
            step(&root.join("skipped"), b"unexpected"),
            Step::DeleteFolderInEachCase {
                path: root.join("cleanup").display().to_string(),
            },
        ],
    };
    let result = executor.run(&plan, Instant::now() + Duration::from_secs(5));
    assert_eq!(
        result.failed,
        Some(StepFailure {
            index: 2,
            fault: Fault::CommandFailed
        })
    );
    assert_eq!(
        result.attempts,
        [StepFailure {
            index: 0,
            fault: Fault::CommandFailed
        }]
    );
    assert_eq!(result.cleanup, []);
    assert_eq!(fs::read(root.join("kept")).unwrap(), b"private");
    assert_eq!(
        fs::metadata(root.join("kept"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(!root.join("skipped").exists());
    assert!(!root.join("cleanup").exists());
}

#[test]
fn a_timed_out_command_cannot_leave_a_background_writer() {
    let (_directory, paths, _listener) = fixture();
    let root = Path::new(&paths.root);
    let marker = root.join("late");
    let running = root.join("running");
    let plan = Plan {
        steps: vec![Step::Run(command(
            &paths,
            r#"printf ready > "$1"; ( "$3" 0.5; printf late > "$2" ) & wait"#,
            &[
                running.to_str().unwrap(),
                marker.to_str().unwrap(),
                &executable("sleep"),
            ],
        ))],
    };
    let mut executor = Executor::open(&paths).unwrap();
    let result = executor.run(&plan, Instant::now() + Duration::from_millis(200));
    assert_eq!(result.failed.unwrap().fault, Fault::LimitSeconds);
    assert!(running.is_file());
    thread::sleep(Duration::from_millis(480));
    assert!(!marker.exists());
}

#[test]
fn application_writes_and_cleanup_cannot_follow_links_into_another_tree() {
    let (_directory, paths, _listener) = fixture();
    let outside = tempfile::tempdir().unwrap();
    let root = Path::new(&paths.root);
    let untouched = outside.path().join("kept");
    fs::write(&untouched, b"outside").unwrap();
    symlink(outside.path(), root.join("alias")).unwrap();
    symlink(&untouched, root.join("local")).unwrap();
    fs::create_dir(root.join("cleanup")).unwrap();
    symlink(outside.path(), root.join("cleanup/alias")).unwrap();
    let plan = Plan {
        steps: vec![
            step(&root.join("local"), b"inside"),
            step(&root.join("alias/kept"), b"bad"),
            Step::DeleteFolderInEachCase {
                path: root.join("cleanup").display().to_string(),
            },
        ],
    };
    let mut executor = Executor::open(&paths).unwrap();
    let result = executor.run(&plan, Instant::now() + Duration::from_secs(5));
    assert_eq!(result.failed.unwrap().index, 1);
    assert_eq!(result.cleanup, []);
    assert_eq!(fs::read(&untouched).unwrap(), b"outside");
    assert_eq!(fs::read(root.join("local")).unwrap(), b"inside");
    assert!(!root.join("cleanup").exists());
}

#[test]
fn replacing_the_engine_socket_with_a_symlink_refuses_a_pending_plan() {
    let (_directory, paths, _listener) = fixture();
    let other = tempfile::tempdir().unwrap();
    let target = other.path().join("different.sock");
    let _unrelated = UnixListener::bind(&target).unwrap();
    let mut executor = Executor::open(&paths).unwrap();
    fs::remove_file(&paths.socket).unwrap();
    symlink(target, &paths.socket).unwrap();
    let local = Path::new(&paths.root).join("receipt");
    let plan = Plan {
        steps: vec![step(&local, b"changed")],
    };
    assert_eq!(
        executor
            .run(&plan, Instant::now() + Duration::from_secs(5))
            .failed
            .unwrap()
            .fault,
        Fault::Socket,
    );
    assert!(!local.exists());
}

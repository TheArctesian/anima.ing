//! The reads of the Git engine (TH116): each thread may read its repository in its own
//! confinement, and the launcher refuses each run that is not exactly a read.

use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use agent_threads::drive::{Mode, Program};
use uuid::Uuid;

use super::of;
use crate::threads::git::place::shared::pack::{closed, list_args};
use crate::threads::git::run::hooks::{Kind, ThreadRun};
use crate::threads::runner::launcher::read::{Confinement, codex_home, confined};
use crate::threads::runner::launcher::start;

const GIT: &str = "/usr/bin/git";

/// What the engine sets for each read.
const QUIET: [(&str, &str); 3] = [
    ("GIT_NO_LAZY_FETCH", "1"),
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_SSH_COMMAND", "false"),
];

/// A repository with one commit in a new folder, and the name of that commit.
fn repository() -> (tempfile::TempDir, String) {
    let folder = tempfile::tempdir().expect("a folder");
    let git = |args: &[&str]| {
        let out = Command::new(GIT)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(folder.path())
            .output()
            .expect("git runs");
        assert!(out.status.success(), "git {args:?}");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    git(&["init", "-q"]);
    std::fs::write(folder.path().join("a"), "a").expect("a file");
    git(&["add", "a"]);
    git(&["commit", "-q", "-m", "one"]);
    let head = git(&["rev-parse", "HEAD"]);
    (folder, head)
}

/// A read as the engine sends it: the closed options, then `own`, with the environment of
/// [`QUIET`] and `more`.
fn read(
    thread: Uuid,
    dir: &Path,
    own: Vec<OsString>,
    more: Vec<(&'static str, OsString)>,
) -> ThreadRun {
    let env = QUIET.map(|(name, value)| (name, OsString::from(value)));
    ThreadRun {
        kind: Kind::Read,
        thread,
        dir: dir.to_owned(),
        program: PathBuf::from(GIT),
        args: closed().into_iter().chain(own).collect(),
        env: env.into_iter().chain(more).collect(),
        input: None,
        within: Duration::from_secs(30),
    }
}

/// The list read of `head` in `dir`.
fn list(thread: Uuid, dir: &Path, head: &str) -> ThreadRun {
    read(thread, dir, list_args(&[head], &[]), Vec::new())
}

/// The pack read of `want` (a list) in `dir`, as the engine makes it: its folder, its objects and
/// its list in the records.
fn pack(thread: Uuid, dir: &Path, want: &str) -> ThreadRun {
    let ours = dir.join(format!(".anima-pack-{}", thread.as_simple()));
    std::fs::create_dir_all(ours.join("objects/pack")).expect("the pack folder");
    let own = dir.join(".records").join(thread.to_string()).join("a.git");
    std::fs::create_dir_all(&own).expect("the records of the thread");
    std::fs::write(own.join("want"), want).expect("the list");
    let args = ["pack-objects", "--revs", "--quiet"].map(OsString::from);
    let args = args
        .into_iter()
        .chain([ours.join("p").into_os_string()])
        .collect();
    let more = vec![
        (
            "GIT_OBJECT_DIRECTORY",
            ours.join("objects").into_os_string(),
        ),
        (
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            dir.join(".git/objects").into_os_string(),
        ),
    ];
    let mut run = read(thread, dir, args, more);
    run.input = Some(own.join("want"));
    run
}

/// A folder with no link in its path, as the engine gives it.
fn real(folder: &tempfile::TempDir) -> PathBuf {
    std::fs::canonicalize(folder.path()).expect("a real path")
}

#[tokio::test]
async fn th116_a_confined_claude_code_thread_reads_its_repository_and_makes_its_pack() {
    let (folder, head) = repository();
    let dir = real(&folder);
    let (rights, thread) = of(Program::ClaudeCode, Mode::Ask, &dir);
    let ended = start(&rights, list(thread, &dir, &head))
        .await
        .expect("it ran");
    assert_eq!(ended.code, Some(0), "{}", ended.said);
    let ended = start(&rights, pack(thread, &dir, &format!("{head}\n"))).await;
    let ended = ended.expect("it ran");
    assert_eq!(ended.code, Some(0), "{}", ended.said);
    let ours = dir.join(format!(".anima-pack-{}", thread.as_simple()));
    let made = std::fs::read_dir(&ours)
        .expect("the pack folder")
        .filter_map(Result::ok)
        .any(|entry| entry.path().extension().is_some_and(|end| end == "pack"));
    assert!(made, "the thread made its pack");
}

#[tokio::test]
async fn th116_each_run_that_is_not_exactly_a_read_is_refused_for_a_confined_thread() {
    let (folder, head) = repository();
    let dir = real(&folder);
    let (rights, thread) = of(Program::ClaudeCode, Mode::AcceptEdits, &dir);
    let refused = async |run: ThreadRun| {
        let ended = start(&rights, run).await;
        assert_eq!(
            ended.map_err(|error| error.kind()),
            Err(ErrorKind::PermissionDenied)
        );
    };
    let mut hook = list(thread, &dir, &head);
    hook.kind = Kind::Hook;
    refused(hook).await;
    let mut env = list(thread, &dir, &head);
    env.env.push(("GIT_DIR", OsString::from("/elsewhere")));
    refused(env).await;
    let mut open = list(thread, &dir, &head);
    open.args.remove(0);
    refused(open).await;
    refused(read(thread, &dir, list_args(&["--all"], &[]), Vec::new())).await;
    refused(read(
        thread,
        &dir,
        ["log", "-p"].map(OsString::from).into(),
        Vec::new(),
    ))
    .await;
}

#[tokio::test]
async fn th116_a_pack_with_a_list_of_more_than_names_or_a_linked_list_is_refused() {
    let (folder, head) = repository();
    let dir = real(&folder);
    let (rights, thread) = of(Program::ClaudeCode, Mode::Ask, &dir);
    let refused = async |run: ThreadRun| {
        let ended = start(&rights, run).await;
        assert_eq!(
            ended.map_err(|error| error.kind()),
            Err(ErrorKind::PermissionDenied)
        );
    };
    refused(pack(thread, &dir, &format!("{head}\n--all\n"))).await;
    let linked = pack(thread, &dir, &format!("{head}\n"));
    let want = linked.input.clone().expect("a list");
    std::fs::remove_file(&want).expect("the list goes");
    std::os::unix::fs::symlink(dir.join("a"), &want).expect("a link in its place");
    let ended = start(&rights, linked).await;
    assert!(ended.is_err(), "a list that is a link is not read");
}

#[tokio::test]
async fn th116_a_confined_codex_thread_whose_copy_is_not_known_reads_nothing() {
    let (folder, head) = repository();
    let dir = real(&folder);
    let (rights, thread) = of(Program::Codex, Mode::Ask, &dir);
    let ended = start(&rights, list(thread, &dir, &head)).await;
    assert_eq!(
        ended.map_err(|error| error.kind()),
        Err(ErrorKind::PermissionDenied)
    );
}

#[test]
fn th116_a_confined_codex_thread_reads_inside_codex_sandbox_with_the_profile_of_its_folder() {
    let thread = Uuid::now_v7();
    let seat = Confinement {
        mode: Mode::Ask,
        program: Program::Codex,
        codex: Some(PathBuf::from("/opt/codex")),
        kept: Some(PathBuf::from("/work/anima")),
    };
    let args = [OsString::from("rev-list")];
    let (program, wrapped, home) =
        confined(thread, &seat, (Path::new(GIT), Path::new("/work"), &args)).expect("a read");
    let profile = format!("anima-reads-{}", thread.as_simple());
    let expected: Vec<OsString> = [
        "sandbox".to_owned(),
        "-P".to_owned(),
        profile.clone(),
        "-c".to_owned(),
        format!("permissions.{profile}.extends=\":workspace\""),
        "-c".to_owned(),
        format!("permissions.{profile}.filesystem={{\"/work/anima\"=\"read\"}}"),
        "-C".to_owned(),
        "/work".to_owned(),
        "--".to_owned(),
        GIT.to_owned(),
        "rev-list".to_owned(),
    ]
    .map(OsString::from)
    .into();
    assert_eq!(
        (program, wrapped, home),
        (PathBuf::from("/opt/codex"), expected, true)
    );
    let full = Confinement {
        mode: Mode::FullAccess,
        ..seat.clone()
    };
    let plain = confined(thread, &full, (Path::new(GIT), Path::new("/work"), &args));
    assert_eq!(plain.expect("a read").0, PathBuf::from(GIT));
    let planning = Confinement {
        mode: Mode::Plan,
        ..seat
    };
    assert!(
        confined(
            thread,
            &planning,
            (Path::new(GIT), Path::new("/work"), &args)
        )
        .is_err()
    );
}

#[test]
fn th116_the_home_of_codex_for_reads_is_private_and_holds_no_configuration() {
    use std::os::unix::fs::PermissionsExt as _;
    let folder = tempfile::tempdir().expect("a folder");
    let home = folder.path().join("codex-reads");
    codex_home(&home).expect("a new home");
    let mode = std::fs::metadata(&home)
        .expect("the home")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
    std::fs::create_dir(home.join("tmp")).expect("what codex writes");
    codex_home(&home).expect("tmp is allowed");
    std::fs::write(
        home.join("config.toml"),
        "sandbox_mode = \"danger-full-access\"",
    )
    .expect("a planted file");
    assert!(
        codex_home(&home).is_err(),
        "a file of configuration is refused"
    );
    let open = folder.path().join("open");
    std::fs::create_dir(&open).expect("a folder");
    std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o755)).expect("its mode");
    assert!(
        codex_home(&open).is_err(),
        "a folder others can read is refused"
    );
}

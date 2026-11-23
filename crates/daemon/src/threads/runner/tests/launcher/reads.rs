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
use crate::threads::git::run::hooks::ThreadRun;
use crate::threads::runner::launcher::read::{CLOSED, Confinement, codex_home, confined};
use crate::threads::runner::launcher::start;

const GIT: &str = "/usr/bin/git";

/// A repository with one commit in a new folder, and the name of that commit.
fn repository() -> (tempfile::TempDir, String) {
    let folder = tempfile::tempdir().expect("a folder");
    let git = |args: &[&str]| {
        let out = Command::new(GIT)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "init.defaultBranch=main",
            ])
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

/// A read with `own` after the closed arguments, for `thread` in `dir`.
fn read(thread: Uuid, dir: &Path, own: &[&str]) -> ThreadRun {
    let args = CLOSED.iter().chain(own).map(OsString::from).collect();
    ThreadRun {
        thread,
        dir: dir.to_owned(),
        program: PathBuf::from(GIT),
        args,
        env: Vec::new(),
        input: None,
        within: Duration::from_secs(30),
    }
}

fn objects(head: &str) -> Vec<&str> {
    vec!["rev-list", "--objects", "--quiet", head, "--not"]
}

#[tokio::test]
async fn th116_a_confined_claude_code_thread_reads_its_repository_and_makes_its_pack() {
    let (folder, head) = repository();
    let (rights, thread) = of(Program::ClaudeCode, Mode::Ask, folder.path());
    let ended = start(&rights, read(thread, folder.path(), &objects(&head))).await;
    let ended = ended.expect("it ran");
    assert_eq!(ended.code, Some(0), "{}", ended.said);
    let pack = folder
        .path()
        .join(format!(".anima-pack-{}", thread.as_simple()));
    std::fs::create_dir(&pack).expect("the pack folder");
    let want = folder.path().join("want");
    std::fs::write(&want, format!("{head}\n")).expect("the list");
    let base = pack.join("p").to_string_lossy().into_owned();
    let mut run = read(
        thread,
        folder.path(),
        &["pack-objects", "--revs", "--quiet", &base],
    );
    run.input = Some(want);
    let ended = start(&rights, run).await.expect("it ran");
    assert_eq!(ended.code, Some(0), "{}", ended.said);
    let made = std::fs::read_dir(&pack)
        .expect("the pack folder")
        .filter_map(Result::ok)
        .any(|entry| entry.path().extension().is_some_and(|end| end == "pack"));
    assert!(made, "the thread made its pack");
}

#[tokio::test]
async fn th116_each_run_that_is_not_exactly_a_read_is_refused_for_a_confined_thread() {
    let (folder, head) = repository();
    let (rights, thread) = of(Program::ClaudeCode, Mode::AcceptEdits, folder.path());
    let refused = async |run: ThreadRun| {
        let ended = start(&rights, run).await;
        assert_eq!(
            ended.map_err(|error| error.kind()),
            Err(ErrorKind::PermissionDenied)
        );
    };
    let mut hook = read(thread, folder.path(), &objects(&head));
    hook.program = PathBuf::from("/bin/sh");
    refused(hook).await;
    let mut env = read(thread, folder.path(), &objects(&head));
    env.env = vec![("GIT_DIR", OsString::from("/elsewhere"))];
    refused(env).await;
    let mut open = read(thread, folder.path(), &objects(&head));
    open.args.remove(0);
    refused(open).await;
    let option = ["rev-list", "--objects", "--quiet", "--all", "--not"];
    refused(read(thread, folder.path(), &option)).await;
    refused(read(
        thread,
        folder.path(),
        &["rev-list", "--objects", "--quiet", &head],
    ))
    .await;
    refused(read(thread, folder.path(), &["log", "-p"])).await;
}

#[tokio::test]
async fn th116_a_pack_is_refused_outside_its_folder_or_with_a_list_of_more_than_names() {
    let (folder, head) = repository();
    let (rights, thread) = of(Program::ClaudeCode, Mode::Ask, folder.path());
    let refused = async |run: ThreadRun| {
        let ended = start(&rights, run).await;
        assert_eq!(
            ended.map_err(|error| error.kind()),
            Err(ErrorKind::PermissionDenied)
        );
    };
    let elsewhere = folder.path().join("p").to_string_lossy().into_owned();
    let mut pack = read(
        thread,
        folder.path(),
        &["pack-objects", "--revs", "--quiet", &elsewhere],
    );
    pack.input = Some(folder.path().join("a"));
    refused(pack).await;
    let base = folder
        .path()
        .join(format!(".anima-pack-{}", thread.as_simple()))
        .join("p")
        .to_string_lossy()
        .into_owned();
    let list = folder.path().join("list");
    std::fs::write(&list, format!("{head}\n--all\n")).expect("a list");
    let mut listed = read(
        thread,
        folder.path(),
        &["pack-objects", "--revs", "--quiet", &base],
    );
    listed.input = Some(list);
    refused(listed).await;
}

#[tokio::test]
async fn th116_a_confined_codex_thread_whose_copy_is_not_known_reads_nothing() {
    let (folder, head) = repository();
    let (rights, thread) = of(Program::Codex, Mode::Ask, folder.path());
    let ended = start(&rights, read(thread, folder.path(), &objects(&head))).await;
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

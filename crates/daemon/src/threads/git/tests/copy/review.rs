//! What the security review of the checked copy found: a run of a thread that obeys the
//! configuration of the repository, a commit that one thread brought and a different thread
//! cannot read, and a pack with the name of a pack that is there.

use agent_threads::git::{GitAction, GitOutcome, GitRefusal};
use uuid::Uuid;

use super::super::{Bench, PERSON, THREAD, refused, target};
use super::as_the_person;
use crate::threads::git::run::hooks::{AsThread, Ended};

#[tokio::test]
async fn a_run_of_the_thread_starts_no_program_that_the_configuration_names() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let ran = folder.with_file_name("RAN");
    // What a confined thread of the same project can write: a remote that promises missing
    // objects, and the program that reaches it.
    let planted = format!(
        "[extensions]\n\tpartialClone = evil\n[remote \"evil\"]\n\turl = ssh://example.invalid/x\n\
         \tpromisor = true\n[core]\n\tsshCommand = touch \"{0}\"; false\n\tfsmonitor = touch \"{0}\"\n",
        ran.display()
    );
    let config = bench.project.join(".git/config");
    let before = std::fs::read_to_string(&config).expect("the configuration");
    std::fs::write(&config, before + &planted).expect("the planted configuration");
    // An ID that the repository does not have, where the worktree of the thread is.
    let entry = format!(".git/worktrees/anima-{}/HEAD", THREAD.as_simple());
    std::fs::write(bench.project.join(entry), format!("{}\n", "1".repeat(40))).expect("HEAD");
    let outcome = bench.call(PERSON, GitAction::Status).await;
    assert!(
        matches!(refused(&outcome), Some(GitRefusal::Failed { .. })),
        "{outcome:?}"
    );
    assert!(bench.engine.turn_ended(target(), 1).await.is_err());
    assert!(!ran.exists(), "the run of the thread started a program");
}

#[tokio::test]
async fn a_thread_that_cannot_read_the_repository_gets_nothing_from_the_copy_of_another() {
    let bench = Bench::new().await;
    // The first thread brings the history of the project into the copy.
    bench.worktree().await;
    // A second thread whose runs cannot read the repository, as a sandbox would have it.
    let blind = AsThread::new(|_| {
        Box::pin(async {
            Ok(Ended {
                code: Some(128),
                said: "fatal: cannot read".to_owned(),
            })
        })
    });
    let second = bench.plan(Uuid::from_u128(7 << 100));
    let made = bench.engine_with(Some(blind)).worktree(second).await;
    assert!(
        matches!(&made, Err(GitRefusal::Failed { said }) if said.contains("could not be read")),
        "{made:?}"
    );
}

/// Gives each pack in a folder of `worktree` the file name `name`.
fn renamed(worktree: &std::path::Path, name: &std::ffi::OsStr) -> std::io::Result<()> {
    let folders = std::fs::read_dir(worktree)?.flatten();
    let inside = folders.filter_map(|out| std::fs::read_dir(out.path()).ok());
    let made = inside.flat_map(|files| files.flatten().map(|file| file.path()));
    for pack in made.filter(|path| path.extension().is_some_and(|kind| kind == "pack")) {
        std::fs::rename(&pack, pack.with_file_name(name))?;
    }
    Ok(())
}

#[tokio::test]
async fn a_pack_of_a_thread_never_takes_the_place_of_a_pack_of_the_copy() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let packs = bench.home.with_file_name("records").join("projects");
    let copy = std::fs::read_dir(packs)
        .expect("the projects")
        .flatten()
        .find(|entry| entry.path().extension().is_some_and(|kind| kind == "git"));
    let packs = copy.expect("the copy").path().join("objects/pack");
    let is_pack = |path: &std::path::PathBuf| path.extension().is_some_and(|kind| kind == "pack");
    let listed = |packs: &std::path::Path| -> Vec<std::path::PathBuf> {
        let all = std::fs::read_dir(packs).expect("the packs").flatten();
        all.map(|entry| entry.path()).filter(is_pack).collect()
    };
    let first = listed(&packs).pop().expect("a pack");
    let (name, bytes) = (
        first.file_name().expect("a name").to_owned(),
        std::fs::read(&first),
    );
    // The thread gives its next pack the name of the pack that is in the copy.
    let honest = as_the_person(None);
    let renaming = AsThread::new(move |run| {
        let (honest, name, dir) = (honest.clone(), name.clone(), run.dir.clone());
        Box::pin(async move {
            let ended = honest.start(run).await?;
            renamed(&dir, &name)?;
            Ok(ended)
        })
    });
    let who = [
        "-c",
        "user.name=Agent",
        "-c",
        "user.email=a@example.invalid",
    ];
    std::fs::write(folder.join("a.txt"), "by the agent\n").expect("a file");
    let made = [&who[..], &["commit", "--quiet", "--all", "--message=agent"]].concat();
    bench.git_in(&folder, &made).await;
    let engine = bench.engine_with(Some(renaming));
    let status = engine.call(PERSON, target(), GitAction::Status).await;
    assert!(matches!(status, GitOutcome::Status(_)), "{status:?}");
    assert_eq!(
        std::fs::read(&first).ok(),
        bytes.ok(),
        "the pack of the copy changed"
    );
    assert_eq!(
        listed(&packs).len(),
        2,
        "the new pack has a name of its own"
    );
}

/// An engine on the records of the bench whose copies can always have `room` bytes.
fn engine_with_room(bench: &Bench, room: u64, as_thread: AsThread) -> crate::threads::git::Engine {
    let records = bench.home.with_file_name("records");
    let home = crate::threads::git::place::Home::at(&bench.home, &records).expect("a home");
    let git = bench.git.clone().with_copy_room(room);
    crate::threads::git::Engine::new(git, home).with_launcher(as_thread)
}

/// The copy of the project of the bench.
fn copy_of(bench: &Bench) -> std::path::PathBuf {
    let projects = bench.home.with_file_name("records").join("projects");
    let all = std::fs::read_dir(projects).expect("the projects").flatten();
    let copy = all
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|kind| kind == "git"));
    copy.expect("the copy")
}

/// Bytes that do not get smaller when they are packed.
fn noise(bytes: usize) -> Vec<u8> {
    let mut at = 0x2545_f491_4f6c_dd1d_u64;
    let next = || {
        at ^= at << 13;
        at ^= at >> 7;
        at ^= at << 17;
        at.to_le_bytes()[0]
    };
    std::iter::repeat_with(next).take(bytes).collect()
}

/// Puts a large file with the name of a pack into each folder of a pack in `worktree`.
fn enlarged(worktree: &std::path::Path) -> std::io::Result<()> {
    let folders = std::fs::read_dir(worktree)?.flatten();
    let of_a_pack = |out: &std::fs::DirEntry| {
        let name = out.file_name();
        name.to_string_lossy().starts_with(".anima-pack-")
    };
    for out in folders.filter(of_a_pack) {
        std::fs::write(out.path().join("large.pack"), noise(4 * 1024 * 1024))?;
    }
    Ok(())
}

#[tokio::test]
async fn a_thread_that_leaves_more_than_the_copy_can_hold_brings_nothing() {
    let bench = Bench::new().await;
    // The run of the thread leaves a second, large file besides its pack.
    let honest = as_the_person(None);
    let greedy = AsThread::new(move |run| {
        let (honest, dir) = (honest.clone(), run.dir.clone());
        Box::pin(async move {
            let ended = honest.start(run).await?;
            enlarged(&dir)?;
            Ok(ended)
        })
    });
    let engine = engine_with_room(&bench, 64 * 1024, greedy);
    let made = engine.worktree(bench.plan(THREAD)).await;
    assert!(
        matches!(&made, Err(GitRefusal::Failed { said }) if said.contains("can hold")),
        "{made:?}"
    );
    let used = engine.disk().first().map_or(0, |used| used.bytes);
    assert!(used < 1024 * 1024, "the copy took {used} bytes");
    // With an honest run the same small copy takes the small project.
    let engine = engine_with_room(&bench, 64 * 1024, as_the_person(None));
    engine
        .worktree(bench.plan(THREAD))
        .await
        .expect("a worktree");
}

#[tokio::test]
async fn a_full_copy_is_made_small_and_refuses_only_when_that_does_not_help() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let engine = engine_with_room(&bench, 64 * 1024, as_the_person(None));
    let copy = copy_of(&bench);
    let who = [
        "-c",
        "user.name=Agent",
        "-c",
        "user.email=a@example.invalid",
    ];
    let commit = |text: &'static str| {
        std::fs::write(folder.join("a.txt"), text).expect("a file");
        [&who[..], &["commit", "--quiet", "--all", "--message=agent"]].concat()
    };
    // An object in the copy that no known commit needs: it goes when the copy is full.
    let loose = bench.home.with_file_name("loose.bin");
    std::fs::write(&loose, noise(512 * 1024)).expect("a file");
    let place = format!("--git-dir={}", copy.display());
    let path = loose.to_string_lossy().into_owned();
    let written = [place.as_str(), "hash-object", "-w", "--", path.as_str()];
    bench.git_in(&bench.home, &written).await;
    bench.git_in(&folder, &commit("second\n")).await;
    let status = engine.call(PERSON, target(), GitAction::Status).await;
    assert!(matches!(status, GitOutcome::Status(_)), "{status:?}");
    let used = engine.disk()[0].bytes;
    assert!(
        used < 256 * 1024,
        "the copy was not made small: {used} bytes"
    );
    // What cannot go: the copy stays full, and nothing more is brought.
    std::fs::write(copy.join("kept.bin"), noise(512 * 1024)).expect("a file");
    bench.git_in(&folder, &commit("third\n")).await;
    let full = engine.call(PERSON, target(), GitAction::Status).await;
    assert!(
        matches!(refused(&full), Some(GitRefusal::Failed { said }) if said.contains("reached the size")),
        "{full:?}"
    );
}

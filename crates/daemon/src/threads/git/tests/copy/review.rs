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

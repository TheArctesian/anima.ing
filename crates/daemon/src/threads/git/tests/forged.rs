//! Objects that are not what their ID says (the security review of design C): `git` does not
//! check the content of an object when it reads one, and an agent can write the object store
//! of the repository. The runner shows and commits only objects that it named itself by the
//! hash of their content.

use std::path::{Path, PathBuf};

use agent_threads::git::{GitAction, GitOutcome, GitRefusal};

use super::trusted::{UNCOMMITTED, commit};
use super::{Bench, PERSON, refused};

impl Bench {
    /// Puts an object with the text `decoy` at the place of the object `id` in the store of
    /// the project, as an agent can with a file copy.
    async fn forge(&self, id: &str, decoy: &str) {
        let scratch = self.home.with_file_name("decoy.txt");
        std::fs::write(&scratch, decoy).expect("a file");
        let path = scratch.to_string_lossy().into_owned();
        let written = ["hash-object", "-w", "--", path.as_str()];
        let written = self.git_in(&self.project, &written).await;
        let objects = self.project.join(".git/objects");
        let file = |id: &str| -> PathBuf { objects.join(&id[..2]).join(&id[2..]) };
        std::fs::create_dir_all(file(id).parent().expect("a folder")).expect("a folder");
        let _ = std::fs::remove_file(file(id));
        std::fs::copy(file(&written), file(id)).expect("the forged object");
    }

    /// The ID that the content of `file` has as an object.
    async fn id_of(&self, file: &Path) -> String {
        let path = file.to_string_lossy().into_owned();
        self.git_in(&self.project, &["hash-object", "--", &path])
            .await
    }
}

#[tokio::test]
async fn a_decoy_at_the_id_of_a_new_file_is_not_what_the_person_is_shown_or_commits() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "the real change\n").expect("a file");
    // The agent knows the ID that its file will get, and puts a different text there first.
    let real = bench.id_of(&folder.join("a.txt")).await;
    bench.forge(&real, "a harmless line\n").await;
    let GitOutcome::Changes(shown) = bench.call(PERSON, UNCOMMITTED).await else {
        panic!("no changes");
    };
    let patch = shown.files[0].patch.as_deref().unwrap_or_default();
    assert!(patch.contains("+the real change"), "{patch}");
    assert!(!patch.contains("harmless"), "{patch}");
    let reviewed = shown.snapshot.expect("a snapshot");
    let committed = bench.call(PERSON, commit(&reviewed, None)).await;
    let GitOutcome::Committed { commit: made, .. } = committed else {
        panic!("no commit: {committed:?}");
    };
    let kept = bench.git_own(&["show", &format!("{made}:a.txt")]).await;
    assert_eq!(kept, "the real change");
}

#[tokio::test]
async fn a_commit_of_the_branch_with_a_forged_file_is_refused_as_the_base_of_anything() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    // The agent commits with its own git, then changes what the committed file says, so that
    // the person would be shown a difference against a text that is not in the commit.
    std::fs::write(folder.join("a.txt"), "by the agent\n").expect("a file");
    let who = [
        "-c",
        "user.name=Agent",
        "-c",
        "user.email=a@example.invalid",
    ];
    let made = [&who[..], &["commit", "--quiet", "--all", "--message=agent"]].concat();
    bench.git_in(&folder, &made).await;
    let committed = bench.id_of(&folder.join("a.txt")).await;
    bench.forge(&committed, "something else\n").await;
    std::fs::write(folder.join("a.txt"), "the next change\n").expect("a file");
    for action in [GitAction::Status, UNCOMMITTED, commit("", None)] {
        let outcome = bench.call(PERSON, action).await;
        assert!(
            matches!(refused(&outcome), Some(GitRefusal::Failed { said }) if said.contains("not what its ID says")),
            "{outcome:?}"
        );
    }
    assert!(bench.engine.turn_ended(super::target(), 1).await.is_err());
}

#[tokio::test]
async fn an_object_forged_after_the_runner_copied_it_changes_nothing() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    // The file of the first commit, forged after the worktree was made.
    let first = bench
        .git_in(&bench.project, &["rev-parse", "HEAD:a.txt"])
        .await;
    bench.forge(&first, "forged base\n").await;
    std::fs::write(folder.join("a.txt"), "one\ntwo\n").expect("a file");
    let GitOutcome::Changes(shown) = bench.call(PERSON, UNCOMMITTED).await else {
        panic!("no changes");
    };
    let patch = shown.files[0].patch.as_deref().unwrap_or_default();
    assert!(patch.contains(" one\n+two"), "{patch}");
    let reverted = bench.call(PERSON, GitAction::Revert { turn: 0 }).await;
    assert!(
        matches!(reverted, GitOutcome::Reverted { .. }),
        "{reverted:?}"
    );
    let text = std::fs::read_to_string(folder.join("a.txt")).expect("the file");
    assert_eq!(text, "one\n");
}

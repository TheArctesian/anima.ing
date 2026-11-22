//! Links that a process of an agent puts in the place of a file or a folder while the runner
//! works (the second security review of design C). `git` looks at a path and then opens it by
//! name; the runner copies each file itself, with no link followed, so nothing outside the
//! worktree is read into a snapshot and nothing is written outside by a revert.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_threads::git::{GitAction, GitOutcome};

use super::trusted::UNCOMMITTED;
use super::{Bench, PERSON};

/// Runs `swap` again and again on a thread of its own until the answer is dropped.
struct Flipping(Arc<AtomicBool>, Option<std::thread::JoinHandle<()>>);

impl Flipping {
    fn start(swap: impl Fn() + Send + 'static) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let seen = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            while !seen.load(Ordering::Relaxed) {
                swap();
            }
        });
        Self(stop, Some(thread))
    }
}

impl Drop for Flipping {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
        if let Some(thread) = self.1.take() {
            let _ = thread.join();
        }
    }
}

/// A folder outside the worktree with a file that no thread may read.
fn outside(bench: &Bench) -> PathBuf {
    let outside = bench.home.with_file_name("outside");
    std::fs::create_dir_all(&outside).expect("a folder");
    std::fs::write(outside.join("secret"), "TOP-SECRET-KEY-MATERIAL\n").expect("a file");
    outside
}

fn link(to: &Path, at: &Path) {
    let _ = std::os::unix::fs::symlink(to, at);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_swapped_for_a_link_brings_no_file_from_outside_into_the_changes() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let secret = outside(&bench).join("secret");
    let swapped = folder.join("x");
    // A file of the same size as the secret, then a link to the secret, then the file again.
    let flipping = Flipping::start(move || {
        let _ = std::fs::remove_file(&swapped);
        let _ = std::fs::write(&swapped, "an innocent line of text\n");
        let _ = std::fs::remove_file(&swapped);
        link(&secret, &swapped);
    });
    for _ in 0..40 {
        let GitOutcome::Changes(shown) = bench.call(PERSON, UNCOMMITTED).await else {
            continue;
        };
        for file in &shown.files {
            let patch = file.patch.as_deref().unwrap_or_default();
            assert!(
                !patch.contains("TOP-SECRET"),
                "a file outside was read: {patch}"
            );
        }
    }
    drop(flipping);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_folder_swapped_for_a_link_gets_no_file_of_a_revert_written_outside() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let outside = outside(&bench);
    std::fs::create_dir_all(folder.join("d")).expect("a folder");
    std::fs::write(folder.join("d/f"), "PAYLOAD\n").expect("a file");
    bench.turn_ended(1).await;
    let (swapped, to) = (folder.join("d"), outside.clone());
    let flipping = Flipping::start(move || {
        let _ = std::fs::remove_dir_all(&swapped);
        let _ = std::fs::remove_file(&swapped);
        link(&to, &swapped);
        let _ = std::fs::remove_file(&swapped);
        let _ = std::fs::create_dir(&swapped);
    });
    for _ in 0..15 {
        // Each answer is fine; what counts is where the files went.
        let _ = bench.call(PERSON, GitAction::Revert { turn: 1 }).await;
        assert!(
            !outside.join("f").exists(),
            "a revert wrote outside the worktree"
        );
    }
    drop(flipping);
    let secret = std::fs::read_to_string(outside.join("secret")).expect("the file");
    assert_eq!(secret, "TOP-SECRET-KEY-MATERIAL\n");
    // With nothing in the way, the revert puts the file back in the worktree.
    let _ = std::fs::remove_file(folder.join("d"));
    let reverted = bench.call(PERSON, GitAction::Revert { turn: 1 }).await;
    assert!(
        matches!(reverted, GitOutcome::Reverted { .. }),
        "{reverted:?}"
    );
    let put_back = std::fs::read_to_string(folder.join("d/f")).expect("the file");
    assert_eq!(put_back, "PAYLOAD\n");
}

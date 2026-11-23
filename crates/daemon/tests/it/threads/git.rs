//! The Git engine of the runner (TH116): the runner makes it at its start, keeps its records
//! in its own folder when the daemon has a home folder of its own, and answers a Git call of a
//! client in a task of its own.

use agent_threads::control::{Action, Outcome};
use agent_threads::git::{GitAction, GitOutcome, GitRefusal};
use uuid::Uuid;

use super::runner::Kit;

#[tokio::test]
async fn th116_a_git_call_is_answered_by_the_engine_of_the_runner() {
    let kit = Kit::new();
    let mut client = kit.client().await;
    let asked = client
        .call(Action::Git {
            thread: Uuid::now_v7(),
            repository: "app".to_owned(),
            action: GitAction::Status,
        })
        .await;
    assert_eq!(
        client.done(asked).await,
        Outcome::Git {
            result: GitOutcome::Refused {
                why: GitRefusal::NoWorktree
            }
        },
        "a thread with no worktree of the runner has no Git"
    );
    let records = kit.threads().join("git-records");
    assert!(
        records.is_dir(),
        "a daemon with a home folder of its own keeps the records in it, never in the place of the installed daemon"
    );
}

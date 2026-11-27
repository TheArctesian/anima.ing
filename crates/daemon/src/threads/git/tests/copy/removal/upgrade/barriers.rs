use super::*;
use crate::threads::git::place::walk::sync_probe;

#[tokio::test]
async fn a_visible_journal_must_be_durable_before_the_move_can_resume() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let own = own_of(&bench);
    let journal = own.with_extension("retiring");
    let records = own.parent().unwrap();
    let fault = sync_probe::fail_for(records, || {});
    assert_sync_failed(&bench).await;
    assert!(journal.is_file());
    assert!(folder.is_dir());
    assert_sync_failed(&bench).await;
    assert!(folder.is_dir());
    drop(fault);
    assert!(matches!(
        bench.call(PERSON, GitAction::Remove).await,
        GitOutcome::Removed { .. }
    ));
}

#[tokio::test]
async fn receipt_replay_must_resync_an_already_unlinked_registration() {
    let bench = Bench::new().await;
    bench.worktree().await;
    let own = own_of(&bench);
    let active = own.with_extension("json");
    let receipt = own.with_extension("removed");
    let monitor = active.clone();
    let fault = sync_probe::fail_when(own.parent().unwrap(), move || !monitor.exists());
    assert_sync_failed(&bench).await;
    assert!(!active.exists());
    assert!(receipt.is_file());
    assert_sync_failed(&bench).await;
    drop(fault);
    assert!(matches!(
        bench.call(PERSON, GitAction::Remove).await,
        GitOutcome::Removed { .. }
    ));
}

async fn assert_sync_failed(bench: &Bench) {
    let result = bench.call(PERSON, GitAction::Remove).await;
    assert!(
        matches!(&result, GitOutcome::Refused { why: agent_threads::git::GitRefusal::Failed { said } }
        if said.contains("injected directory sync failure")),
        "{result:?}"
    );
}

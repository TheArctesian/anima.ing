use chrono::Utc;

use super::*;
use crate::remote_code::launch::dispatch::tests::started_fixture;

const OLD: CredentialStamp = [3; 32];
const NEW: CredentialStamp = [4; 32];

#[test]
fn publication_requires_a_correlated_current_receipt_across_restart() {
    let (_dir, home, launch, _) = started_fixture();
    let request = launch.request();
    let hub = Origin::default_hub();
    let queue = |store: &Store, credential, revision| {
        store
            .request_publication(request, credential, || Some(revision))
            .unwrap()
    };
    assert_eq!(
        queue(&Store::at(&home), OLD, 10),
        PublicationProgress::Waiting
    );
    let store = Store::at(&home);
    assert_eq!(
        store.publication_requests(request.device, &hub).unwrap()[&request.runner],
        10
    );

    let mut sent = sent(request.runner, 11);
    let mut received = received(request.runner, 10);
    store
        .acknowledge_publication((request.device, &hub, OLD), &sent, &received)
        .unwrap();
    received.summaries[0].revision = 20;
    received.summaries[0].runner_id = Uuid::now_v7();
    store
        .acknowledge_publication((request.device, &hub, OLD), &sent, &received)
        .unwrap();
    assert_eq!(queue(&store, OLD, 21), PublicationProgress::Waiting);

    received.summaries[0].runner_id = request.runner;
    store
        .acknowledge_publication((request.device, &hub, OLD), &sent, &received)
        .unwrap();
    let store = Store::at(&home);
    assert_eq!(queue(&store, OLD, 21), PublicationProgress::Accepted);
    assert_eq!(queue(&store, NEW, 21), PublicationProgress::Waiting);

    sent.summaries[0].revision = 21;
    received.summaries[0].revision = 21;
    store
        .acknowledge_publication((request.device, &hub, OLD), &sent, &received)
        .unwrap();
    assert_eq!(queue(&store, NEW, 22), PublicationProgress::Waiting);
    store
        .acknowledge_publication((request.device, &hub, NEW), &sent, &received)
        .unwrap();
    let store = Store::at(&home);
    assert_eq!(queue(&store, NEW, 22), PublicationProgress::Accepted);
    store.publication_reported(request, NEW).unwrap();
    assert_eq!(
        queue(&Store::at(&home), NEW, 23),
        PublicationProgress::Complete
    );
}

fn sent(runner_id: Uuid, revision: i64) -> ThreadSummaries {
    let now = Utc::now();
    ThreadSummaries {
        count: 1,
        count_revision: revision,
        summaries: vec![ThreadSummary {
            runner_id,
            program: "codex".to_owned(),
            state: "idle".to_owned(),
            title: "fixture".to_owned(),
            started_at: now,
            activity_at: now,
            revision,
            repository: None,
        }],
    }
}

fn received(runner_id: Uuid, revision: i64) -> ThreadSummariesReceipt {
    ThreadSummariesReceipt {
        count_revision: revision,
        summaries: vec![ThreadSummaryReceipt {
            runner_id,
            id: Uuid::now_v7(),
            revision,
        }],
    }
}

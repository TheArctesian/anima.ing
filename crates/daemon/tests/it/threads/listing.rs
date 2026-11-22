//! How the person lists the threads (TH46, TH61): kept by the runner across its restart, never a
//! change of a thread that runs.

use agent_threads::control::{Action, Listing, ListingChange, Outcome, Refusal, RunState};
use agent_threads::drive::{Mode, Program};
use uuid::Uuid;

use super::runner::{CLAUDE, Client, Kit};

async fn change(client: &mut Client, thread: Uuid, change: ListingChange) -> Outcome {
    let id = client.call(Action::ChangeListing { thread, change }).await;
    client.done(id).await
}

async fn listings(client: &mut Client) -> Vec<Listing> {
    let id = client.call(Action::Listings).await;
    let Outcome::Listings { listings } = client.done(id).await else {
        panic!("listings")
    };
    listings
}

async fn state_of(client: &mut Client, thread: Uuid) -> Option<RunState> {
    let list = client.call(Action::List).await;
    let Outcome::Threads { threads, .. } = client.done(list).await else {
        panic!("threads")
    };
    threads
        .iter()
        .find(|row| row.id == thread)
        .map(|row| row.state)
}

#[tokio::test]
async fn th46_a_pin_and_a_title_are_kept_after_a_restart_and_a_bad_title_is_refused() {
    let mut kit = Kit::with(CLAUDE);
    let mut client = kit.client().await;
    // A thread of a session file: the runner did not start it.
    let thread = Uuid::now_v7();
    change(&mut client, thread, ListingChange::Pin { on: true }).await;
    let title = Some("Fix the login".to_owned());
    change(&mut client, thread, ListingChange::Rename { title }).await;
    let hidden = Some("pay\u{202E}evil".to_owned());
    assert_eq!(
        change(&mut client, thread, ListingChange::Rename { title: hidden }).await,
        Outcome::Refused {
            why: Refusal::BadValue
        }
    );
    drop(client);

    kit.end_runner();
    kit.start();
    let mut client = kit.client().await;
    let kept = listings(&mut client).await;
    assert_eq!(kept.len(), 1);
    assert!(kept[0].pinned);
    assert_eq!(kept[0].title.as_deref(), Some("Fix the login"));

    change(&mut client, thread, ListingChange::Pin { on: false }).await;
    change(&mut client, thread, ListingChange::Rename { title: None }).await;
    assert!(
        listings(&mut client).await.is_empty(),
        "a plain listing is not kept"
    );
}

#[tokio::test]
async fn th46_a_thread_that_runs_is_not_removed_and_a_stopped_one_leaves_the_list() {
    let kit = Kit::with(CLAUDE);
    let mut client = kit.client().await;
    let start = client
        .call(Action::Start {
            program: Program::ClaudeCode,
            folder: kit.path("work"),
            mode: Mode::FullAccess,
            model: None,
            effort: None,
            prompt: "Run the command".to_owned(),
            program_path: None,
            draft: None,
            attachments: Vec::new(),
        })
        .await;
    let Outcome::Started { thread } = client.done(start).await else {
        panic!("the thread started");
    };
    assert_eq!(
        change(&mut client, thread, ListingChange::Remove).await,
        Outcome::Refused {
            why: Refusal::Running
        }
    );
    let stop = client.call(Action::Stop { thread }).await;
    client.done(stop).await;
    for _ in 0..40 {
        if state_of(&mut client, thread).await == Some(RunState::Stopped) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }

    let Outcome::Listings { listings } = change(&mut client, thread, ListingChange::Remove).await
    else {
        panic!("removed");
    };
    assert!(
        listings
            .iter()
            .any(|listing| listing.thread == thread && listing.removed)
    );
    assert_eq!(
        state_of(&mut client, thread).await,
        None,
        "the runner forgot it"
    );
}

#[tokio::test]
async fn th61_a_settle_and_a_snooze_are_kept_after_a_restart_and_a_bad_snooze_is_refused() {
    let mut kit = Kit::with(CLAUDE);
    let mut client = kit.client().await;
    let (settled, snoozed) = (Uuid::now_v7(), Uuid::now_v7());
    change(&mut client, settled, ListingChange::Settle { on: true }).await;
    let until = chrono::Utc::now() + chrono::TimeDelta::hours(3);
    change(&mut client, snoozed, ListingChange::Snooze { until }).await;
    let past = chrono::Utc::now() - chrono::TimeDelta::hours(1);
    assert_eq!(
        change(&mut client, snoozed, ListingChange::Snooze { until: past }).await,
        Outcome::Refused {
            why: Refusal::BadValue
        }
    );
    drop(client);

    kit.end_runner();
    kit.start();
    let mut client = kit.client().await;
    let kept = listings(&mut client).await;
    let of = |thread: Uuid| {
        kept.iter()
            .find(|listing| listing.thread == thread)
            .cloned()
    };
    assert!(of(settled).is_some_and(|listing| listing.marks.settled_at.is_some()));
    let snooze = of(snoozed)
        .expect("kept")
        .marks
        .snoozed_until
        .expect("snoozed");
    assert_eq!(snooze.timestamp_micros(), until.timestamp_micros());

    change(&mut client, snoozed, ListingChange::Wake).await;
    let woken = listings(&mut client).await;
    let marks = woken
        .iter()
        .find(|listing| listing.thread == snoozed)
        .expect("the wake is kept")
        .marks;
    assert!(marks.snoozed_until.is_none() && marks.woke_at.is_some());
}

use super::*;
use agent_threads::managed::provider_login::Provider;
use chrono::{TimeDelta, Utc};
use std::time::Duration;

fn login(session: u128) -> Login {
    Login {
        session: Uuid::from_u128(session),
        principal: Uuid::from_u128(2),
        workspace: Uuid::from_u128(3),
        generation: 7,
        provider: Provider::Claude,
        valid_until: Utc::now() + TimeDelta::seconds(15),
        expires_at: Utc::now() + TimeDelta::minutes(10),
    }
}
fn code() -> Zeroizing<String> {
    Zeroizing::new("synthetic-code#synthetic-state".into())
}

#[tokio::test]
async fn only_the_owning_human_can_reply_once_after_a_challenge() {
    let replies = Replies::default();
    let login = login(1);
    let device = DeviceId::from(Uuid::from_u128(4));
    let mut lease = replies
        .open(device, &login, Instant::now() + Duration::from_secs(60))
        .unwrap();
    assert!(!replies.submit(login.session, login.principal, code()));
    assert!(lease.ready());
    assert!(
        replies
            .binding(login.session, Uuid::from_u128(99))
            .is_none()
    );
    assert!(!replies.submit(login.session, Uuid::from_u128(99), code()));
    let (bound_device, bound) = replies.binding(login.session, login.principal).unwrap();
    assert_eq!(bound_device, device);
    assert_eq!((bound.workspace, bound.generation), (login.workspace, 7));
    assert!(replies.submit(login.session, login.principal, code()));
    assert!(!replies.submit(login.session, login.principal, code()));
    assert_eq!(
        lease.receive.recv().await.unwrap().as_str(),
        "synthetic-code#synthetic-state"
    );
    drop(lease);
    assert!(replies.binding(login.session, login.principal).is_none());
}

#[tokio::test(start_paused = true)]
async fn a_closed_stream_or_elapsed_deadline_cannot_accept_a_reply() {
    let replies = Replies::default();
    let login = login(1);
    let device = DeviceId::from(Uuid::from_u128(4));
    assert!(replies.open(device, &login, Instant::now()).is_none());
    let lease = replies
        .open(device, &login, Instant::now() + Duration::from_secs(60))
        .unwrap();
    assert!(lease.ready());
    drop(lease);
    assert!(!replies.submit(login.session, login.principal, code()));
    assert!(replies.binding(login.session, login.principal).is_none());
    let lease = replies
        .open(device, &login, Instant::now() + Duration::from_secs(1))
        .unwrap();
    assert!(lease.ready());
    tokio::time::advance(Duration::from_secs(1)).await;
    assert!(replies.binding(login.session, login.principal).is_none());
    assert!(!replies.submit(login.session, login.principal, code()));
    assert!(!lease.ready());
}

#[test]
fn per_person_capacity_is_released_when_a_stream_closes() {
    let replies = Replies::default();
    let device = DeviceId::from(Uuid::from_u128(4));
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut leases: Vec<_> = (10..14)
        .map(|id| replies.open(device, &login(id), deadline).unwrap())
        .collect();
    assert!(replies.open(device, &login(20), deadline).is_none());
    drop(leases.pop());
    assert!(replies.open(device, &login(20), deadline).is_some());
}

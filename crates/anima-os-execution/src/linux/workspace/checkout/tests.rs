use super::{ManagedCredential, Receipt, Source, authorize_credential, record::Record};
use agent_threads::managed::workspace::CheckoutGrant;
use chrono::{TimeDelta, Utc};
use uuid::Uuid;
use zeroize::Zeroizing;

#[test]
fn only_the_pending_private_checkout_accepts_its_repository_credential() {
    let mut state = Record::new(Uuid::from_u128(1), 1 << 30);
    state.principal = Some(Uuid::from_u128(2));
    let repository_id = Uuid::from_u128(3);
    let source = Source::from(agent_threads::managed::workspace::Source {
        owner: "example".to_owned(),
        repository: "app".to_owned(),
        commit: "a".repeat(40),
        repository_id: Some(repository_id),
    });
    let make_credential = |id| {
        ManagedCredential::receive(
            CheckoutGrant {
                repository_id: id,
                expires_at: Utc::now() + TimeDelta::seconds(30),
            },
            Zeroizing::new(b"synthetic-workspace-token".to_vec()),
        )
        .unwrap()
    };
    let correct = make_credential(repository_id);
    let wrong = make_credential(Uuid::from_u128(4));

    state.checkout = Some(Receipt::pending(source.clone()));
    assert!(authorize_credential(&state, None).is_err());
    assert!(authorize_credential(&state, Some(&wrong)).is_err());
    authorize_credential(&state, Some(&correct)).unwrap();

    state.checkout = Some(Receipt::restored(source));
    assert!(authorize_credential(&state, Some(&correct)).is_err());
    authorize_credential(&state, None).unwrap();
}

//! The Linux token file never waits, so the wait (R-322) comes from [`unstored`](super::unstored).

use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::wire::DaemonToken;
use reqwest::Url;
use uuid::Uuid;

use super::Pusher;
use super::unstored::{self, Stored};
use crate::config::home::Home;
use crate::config::origin::Origin;
use crate::hub::{HubClient, TIMEOUT};
use crate::keychain_wait::MARK;
use crate::token_store::{Credential, TokenStore, TokenStoreError};

fn credential(seed: u8) -> Credential {
    Credential {
        token: DaemonToken::from_random([seed; 32]),
        expires_at: DateTime::UNIX_EPOCH + TimeDelta::days(365),
        hub: Origin::parse("https://hub.example").expect("origin"),
    }
}

fn waiting(home: &Home) -> Pusher {
    let store = TokenStore::open(home);
    store.save(&credential(1)).expect("stored");
    let hub =
        HubClient::new(Url::parse("http://127.0.0.1:9").expect("url"), TIMEOUT).expect("a client");
    let mut pusher =
        Pusher::new(hub, Uuid::from_u128(1), credential(1), store).with_marks(home.clone());
    let now: DateTime<Utc> = DateTime::UNIX_EPOCH;
    let Stored::Waits { unstored, .. } =
        unstored::store(credential(2), now, |_| Err(TokenStoreError::KeychainWaits))
    else {
        panic!("a wait");
    };
    pusher.unstored = Some(unstored);
    pusher.show_waits();
    assert!(home.dir().join(MARK).is_file(), "the mark is put");
    pusher
}

#[tokio::test]
async fn met_013_a_changed_token_store_drops_the_renewed_token_that_waits_and_its_mark() {
    let dir = tempfile::tempdir().expect("a folder");
    let home = Home::at(dir.path().join("daemon"));
    let mut pusher = waiting(&home);
    TokenStore::open(&home)
        .save(&credential(3))
        .expect("stored");

    assert!(pusher.adopt_changed_token().await);
    assert!(pusher.credential.token.hash() == credential(3).token.hash());
    assert!(
        pusher.unstored.is_none(),
        "the token that waited is dropped"
    );
    pusher.show_waits();
    assert!(!home.dir().join(MARK).exists(), "the mark is taken away");
}

#[tokio::test]
async fn met_013_a_token_store_that_holds_the_current_token_keeps_the_one_that_waits() {
    let dir = tempfile::tempdir().expect("a folder");
    let home = Home::at(dir.path().join("daemon"));
    let mut pusher = waiting(&home);

    assert!(!pusher.adopt_changed_token().await);
    assert!(pusher.unstored.is_some(), "still waits to be stored");
    pusher.show_waits();
    assert!(home.dir().join(MARK).is_file(), "the mark stays");
}

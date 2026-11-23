use super::{SECRET, use_memory_store};
use crate::keystore::{
    retired::forget_updater_key,
    token::{SERVICE, TokenKind, read_token, write_token},
};

/// Spelled out, not imported, so a wrong name in the code fails.
const WRITTEN_BY_T2: [&str; 2] = ["updater-key", "updater-key-public"];

#[test]
fn r_117_d_41_the_retired_updater_key_entries_are_deleted_and_nothing_else() {
    let store = use_memory_store();
    let origin = "https://retired-key.anima.test";
    write_token(TokenKind::Access, origin, SECRET).unwrap();
    for account in WRITTEN_BY_T2 {
        let key = (SERVICE.to_owned(), account.to_owned());
        store.lock().unwrap().insert(key, "sealed".to_owned());
    }
    forget_updater_key().unwrap();
    for account in WRITTEN_BY_T2 {
        let key = (SERVICE.to_owned(), account.to_owned());
        assert!(!store.lock().unwrap().contains_key(&key), "{account}");
    }
    assert_eq!(
        read_token(TokenKind::Access, origin).unwrap().as_deref(),
        Some(SECRET)
    );
    forget_updater_key().unwrap();
}

//! The platform boundary is the keyring crate's credential interface; no real credential is read.
use anima_cli::{
    auth::{token_file::TokenFile, token_store::TokenStore},
    hub::{Origin, Token},
};
use keyring::credential::{Credential, CredentialApi, CredentialBuilderApi};
use std::{
    any::Any,
    sync::{Arc, Mutex},
};
use zeroize::Zeroizing;

#[derive(Default)]
struct KeyringState {
    locked: bool,
    secret: Option<Vec<u8>>,
}
#[derive(Clone)]
struct Keyring(Arc<Mutex<KeyringState>>);
impl Keyring {
    fn state(&self) -> keyring::Result<std::sync::MutexGuard<'_, KeyringState>> {
        let state = self.0.lock().unwrap();
        if state.locked {
            return Err(keyring::Error::NoStorageAccess(Box::new(
                std::io::Error::other("synthetic locked store"),
            )));
        }
        Ok(state)
    }
}
impl CredentialApi for Keyring {
    fn set_secret(&self, value: &[u8]) -> keyring::Result<()> {
        self.state()?.secret = Some(value.to_vec());
        Ok(())
    }
    fn get_secret(&self) -> keyring::Result<Vec<u8>> {
        self.state()?.secret.clone().ok_or(keyring::Error::NoEntry)
    }
    fn delete_credential(&self) -> keyring::Result<()> {
        self.state()?.secret = None;
        Ok(())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl CredentialBuilderApi for Keyring {
    fn build(&self, _: Option<&str>, _: &str, _: &str) -> keyring::Result<Box<Credential>> {
        Ok(Box::new(self.clone()))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[test]
fn a_fallback_login_and_logout_cannot_restore_an_older_keyring_session() {
    let state = Arc::new(Mutex::new(KeyringState::default()));
    keyring::set_default_credential_builder(Box::new(Keyring(state.clone())));
    let room = tempfile::tempdir().unwrap();
    let store = TokenStore::for_system(Some(TokenFile::new(room.path())), false);
    let origin = Origin::parse("https://hub.example").unwrap();
    let first = Token::new(Zeroizing::new("first-independent-session".to_owned())).unwrap();
    let second = Token::new(Zeroizing::new("second-independent-session".to_owned())).unwrap();
    store.write(&origin, &first).unwrap();
    state.lock().unwrap().locked = true;
    #[cfg(target_os = "linux")]
    store.write(&origin, &second).unwrap();
    #[cfg(not(target_os = "linux"))]
    TokenStore::File(TokenFile::new(room.path()))
        .write(&origin, &second)
        .unwrap();
    state.lock().unwrap().locked = false;
    assert_eq!(
        store.read(&origin).unwrap().unwrap().expose(),
        second.expose()
    );
    state.lock().unwrap().locked = true;
    store.delete(&origin).unwrap();
    state.lock().unwrap().locked = false;
    assert!(store.read(&origin).unwrap().is_none());
    assert_eq!(
        state.lock().unwrap().secret.as_deref(),
        Some(first.expose().as_bytes()),
        "the inaccessible entry still exists but is never authoritative"
    );
}

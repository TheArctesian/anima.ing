use keyring::Entry;
use tauri::Webview;
use zeroize::Zeroizing;

use super::CloudError;
use crate::keystore::SERVICE;

/// One refresh token per client ID in the OS key store (HMD-034); it never reaches the web view, a file or a log (LOG-007).
fn entry(client_id: &str) -> Result<Entry, CloudError> {
    Entry::new(&format!("{SERVICE}.cloud"), client_id).map_err(|_| CloudError::Keystore)
}

pub(super) async fn read(
    _: &Webview,
    client_id: &'static str,
) -> Result<Option<Zeroizing<String>>, CloudError> {
    blocking(move || read_password(client_id)).await
}

fn read_password(client_id: &str) -> Result<Option<Zeroizing<String>>, CloudError> {
    match entry(client_id)?.get_password() {
        Ok(token) => Ok(Some(Zeroizing::new(token))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(CloudError::Keystore),
    }
}

/// `WorkOS` may rotate the token at each exchange.
pub(super) async fn write(
    _: &Webview,
    client_id: &'static str,
    token: &Zeroizing<String>,
) -> Result<(), CloudError> {
    let token = token.clone();
    blocking(move || {
        entry(client_id)?
            .set_password(&token)
            .map_err(|_| CloudError::Keystore)
    })
    .await
}

pub(super) async fn clear(_: &Webview, client_id: &'static str) -> Result<(), CloudError> {
    blocking(move || clear_password(client_id)).await
}

fn clear_password(client_id: &str) -> Result<(), CloudError> {
    match entry(client_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(CloudError::Keystore),
    }
}

async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, CloudError> + Send + 'static,
) -> Result<T, CloudError> {
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|_| CloudError::Keystore)?
}

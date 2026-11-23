use std::sync::PoisonError;

use tauri::Webview;
use zeroize::Zeroizing;

use super::{Cloud, CloudError, client_id, session, workos};

impl Cloud {
    pub(super) fn set_access(&self, token: Option<Zeroizing<String>>) {
        *self.access.lock().unwrap_or_else(PoisonError::into_inner) = token;
    }

    fn access(&self) -> Option<Zeroizing<String>> {
        self.access
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The access token stays in memory and the refresh token goes to the key store (HMD-034).
    pub(super) async fn keep(
        &self,
        webview: &Webview,
        id: &'static str,
        tokens: &workos::Tokens,
    ) -> Result<(), CloudError> {
        let _guard = self.session.lock().await;
        self.save(webview, id, tokens).await
    }

    async fn save(
        &self,
        webview: &Webview,
        id: &'static str,
        tokens: &workos::Tokens,
    ) -> Result<(), CloudError> {
        session::write(webview, id, &tokens.refresh).await?;
        self.set_access(Some(tokens.access.clone()));
        Ok(())
    }

    pub(super) async fn refreshed(
        &self,
        webview: &Webview,
        id: &'static str,
    ) -> Result<(), CloudError> {
        // An exchange can consume its input token, so the read, exchange and replacement share one lock.
        let _guard = self.session.lock().await;
        let Some(stored) = session::read(webview, id).await? else {
            return Err(CloudError::Unauthenticated);
        };
        match workos::refresh(&self.http, id, &stored).await {
            Ok(tokens) => self.save(webview, id, &tokens).await,
            Err(CloudError::Unauthenticated) => {
                self.set_access(None);
                session::clear(webview, id).await?;
                Err(CloudError::Unauthenticated)
            }
            Err(other) => Err(other),
        }
    }

    pub(super) async fn with_access<T, F, R>(
        &self,
        webview: &Webview,
        call: F,
    ) -> Result<T, CloudError>
    where
        F: Fn(Zeroizing<String>) -> R,
        R: std::future::Future<Output = Result<T, CloudError>>,
    {
        let id = client_id()?;
        if self.access().is_none() {
            self.refreshed(webview, id).await?;
        }
        let token = self.access().ok_or(CloudError::Unauthenticated)?;
        match call(token).await {
            Err(CloudError::Unauthenticated) => {
                self.refreshed(webview, id).await?;
                let token = self.access().ok_or(CloudError::Unauthenticated)?;
                call(token).await
            }
            other => other,
        }
    }
}

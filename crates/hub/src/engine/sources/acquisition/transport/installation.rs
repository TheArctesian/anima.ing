//! The two HTTP operations that manage GitHub installation tokens.
use provider_github::CheckoutScope;
use reqwest::{
    Url,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderValue},
};

use super::{Answer, Fault, MEDIA_TYPE, Transport, bearer, read};

impl Transport {
    pub(in crate::engine::sources) async fn mint(
        &self,
        url: Url,
        app_token: &str,
        checkout: Option<CheckoutScope>,
    ) -> Result<Answer, Fault> {
        let mut sent = self
            .client
            .post(url)
            .header(AUTHORIZATION, bearer(app_token.as_bytes())?)
            .header(ACCEPT, HeaderValue::from_static(MEDIA_TYPE));
        if let Some(scope) = checkout {
            let body = serde_json::to_vec(&scope.request()).map_err(|_| Fault::Refused)?;
            sent = sent
                .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                .body(body);
        }
        read(sent.send().await.map_err(|_| Fault::Network)?).await
    }

    pub(in crate::engine::sources::acquisition) async fn revoke_installation(
        &self,
        url: Url,
        token: &[u8],
    ) -> Result<(), Fault> {
        let response = self
            .client
            .delete(url)
            .header(AUTHORIZATION, bearer(token)?)
            .header(ACCEPT, HeaderValue::from_static(MEDIA_TYPE))
            .send()
            .await
            .map_err(|_| Fault::Network)?;
        match response.status().as_u16() {
            204 | 401 => Ok(()),
            _ => Err(Fault::Refused),
        }
    }
}

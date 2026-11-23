//! The calls of the Anima.ing cloud (HMD-043), as the hub plan of the site (2026-11-17) defines them.

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::CloudError;

/// The hub of the account: ready, or a job of the cloud that makes or wakes it (HMD-025).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HubState {
    Ready,
    Making,
    Waking,
}

#[derive(Deserialize)]
struct HubAnswer {
    state: HubState,
}

/// The link invite (none when already joined) and the one-time claim (R-129, HMD-040): secrets for the main web view, so no `Debug`.
#[derive(Deserialize, Serialize)]
pub struct HubAccess {
    invite: Option<String>,
    claim: String,
}

#[cfg(desktop)]
impl HubAccess {
    pub fn is_joined(&self) -> bool {
        self.invite.is_none()
    }
}

impl Drop for HubAccess {
    fn drop(&mut self) {
        if let Some(invite) = self.invite.as_mut() {
            zeroize::Zeroize::zeroize(invite);
        }
        zeroize::Zeroize::zeroize(&mut self.claim);
    }
}

#[derive(Deserialize)]
struct ErrorAnswer {
    error: ErrorCode,
}

#[derive(Deserialize)]
struct ErrorCode {
    code: String,
}

pub(super) async fn hub(http: &Client, base: &str, access: &str) -> Result<HubState, CloudError> {
    let response = http
        .post(format!("{base}/v1/hub"))
        .bearer_auth(access)
        // The site refuses a POST that is not JSON (SvelteKit's cross-site form check).
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body("{}")
        .send()
        .await?;
    let (status, body) = (response.status(), response.bytes().await?);
    if !status.is_success() {
        return Err(refusal(status, &body));
    }
    let answer: HubAnswer = serde_json::from_slice(&body).map_err(|_| CloudError::Refused)?;
    Ok(answer.state)
}

pub(super) async fn hub_access(
    http: &Client,
    base: &str,
    access: &str,
    device: &str,
) -> Result<HubAccess, CloudError> {
    let body = serde_json::json!({ "device": device }).to_string();
    let response = http
        .post(format!("{base}/v1/hub/access"))
        .bearer_auth(access)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .await?;
    let (status, body) = (
        response.status(),
        Zeroizing::new(response.bytes().await?.to_vec()),
    );
    if !status.is_success() {
        return Err(refusal(status, &body));
    }
    serde_json::from_slice(&body).map_err(|_| CloudError::Refused)
}

/// `POST /v1/hub/usage` (HMD-073, `contracts/account-usage.md`); the body goes to the page unchanged, and the page checks its form.
pub(super) async fn hub_usage(
    http: &Client,
    base: &str,
    access: &str,
) -> Result<serde_json::Value, CloudError> {
    let response = http
        .post(format!("{base}/v1/hub/usage"))
        .bearer_auth(access)
        .timeout(USAGE_WAIT)
        .send()
        .await?;
    let (status, body) = (response.status(), response.bytes().await?);
    if !status.is_success() {
        return Err(refusal(status, &body));
    }
    serde_json::from_slice(&body).map_err(|_| CloudError::Refused)
}

const USAGE_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

pub(super) fn refusal(status: StatusCode, body: &[u8]) -> CloudError {
    if status == StatusCode::UNAUTHORIZED {
        return CloudError::Unauthenticated;
    }
    if status.is_server_error() {
        return CloudError::Offline;
    }
    let code = serde_json::from_slice::<ErrorAnswer>(body).map(|answer| answer.error.code);
    match (status, code.as_deref()) {
        (StatusCode::PAYMENT_REQUIRED, _) => CloudError::PaymentFailed,
        (StatusCode::CONFLICT, Ok("not_ready")) => CloudError::NotReady,
        _ => CloudError::Refused,
    }
}

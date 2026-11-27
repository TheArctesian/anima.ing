//! Browser approval at the account site. No cookies, redirects, proxies or response bodies in errors.
use super::HubError;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use zeroize::Zeroizing;

const SITE: &str = "https://anima.ing";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Started {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
}

#[derive(Deserialize)]
pub struct Polled {
    pub status: String,
    pub interval: u64,
    pub access: Option<Access>,
}

#[derive(Deserialize)]
pub struct Access {
    pub invite: Option<String>,
    pub claim: String,
    pub hub: String,
}

impl Drop for Access {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.invite.zeroize();
        self.claim.zeroize();
    }
}

pub(crate) async fn start(computer: &str) -> Result<Started, HubError> {
    post(
        "/v1/cli/sign-in",
        &serde_json::json!({"computer": computer}),
    )
    .await
}

pub(crate) async fn poll(device_code: &str) -> Result<Polled, HubError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Poll<'a> {
        device_code: &'a str,
    }
    post("/v1/cli/sign-in/poll", &Poll { device_code }).await
}

async fn post<T: DeserializeOwned>(path: &str, body: &impl Serialize) -> Result<T, HubError> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(super::TIMEOUT)
        .build()
        .map_err(|_| HubError::Setup)?;
    let mut response = client
        .post(format!("{SITE}{path}"))
        .header("content-type", "application/json")
        .body(bytes::Bytes::from_owner(super::encode(body)))
        .send()
        .await
        .map_err(|_| HubError::Unreachable {
            origin: SITE.to_owned(),
            reason: "the account site did not answer".to_owned(),
        })?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(HubError::Malformed { status });
    }
    let mut bytes = Zeroizing::new(Vec::new());
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| HubError::Malformed { status })?
    {
        if bytes.len() + chunk.len() > 64 * 1024 {
            return Err(HubError::Malformed { status });
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| HubError::Malformed { status })
}

#[cfg_attr(
    not(unix),
    expect(
        clippy::unused_async,
        reason = "shared async interface; socket polling awaits only on Unix"
    )
)]
pub(crate) async fn wait_for_link(origin: &super::Origin) -> Result<(), HubError> {
    #[cfg(unix)]
    if let Some(path) = origin.socket_path() {
        let client = reqwest::Client::builder()
            .unix_socket(path)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .map_err(|_| HubError::Setup)?;
        for _ in 0..30 {
            if client
                .get("http://localhost/health")
                .send()
                .await
                .is_ok_and(|response| response.status().is_success())
            {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
    Err(HubError::Unreachable {
        origin: origin.as_text(),
        reason: "the private link is not ready".to_owned(),
    })
}

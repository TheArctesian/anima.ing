use super::{
    Error, Result,
    http::{post, text},
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

const CLIENT: &str = "client_01M7M1VFKREYV6H5714SDVS4E6";
#[derive(Default)]
pub struct Account {
    pending: Option<Zeroizing<String>>,
    pub access: Option<Zeroizing<String>>,
    next: Option<Instant>,
    deadline: Option<Instant>,
    interval: u64,
}

impl Account {
    pub async fn start(&mut self) -> Result<Value> {
        *self = Self::default();
        let answer = post(
            "https://api.workos.com/user_management/authorize/device",
            &json!({"client_id":CLIENT}),
            None,
            true,
        )
        .await?;
        if answer.get("failure").is_some() {
            return Err(Error::Refused(
                "Sign-in is unavailable for this account configuration.",
            ));
        }
        let url = text(&answer, "verification_uri_complete")?;
        let parsed = reqwest::Url::parse(url).map_err(|_| {
            Error::Refused("The sign-in service returned an invalid browser address.")
        })?;
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(Error::Refused(
                "The sign-in service returned an invalid browser address.",
            ));
        }
        self.interval = answer["interval"].as_u64().unwrap_or(5).clamp(5, 60);
        self.pending = Some(Zeroizing::new(text(&answer, "device_code")?.into()));
        self.next = Some(Instant::now() + Duration::from_secs(self.interval));
        self.deadline = Some(
            Instant::now()
                + Duration::from_secs(answer["expires_in"].as_u64().unwrap_or(600).min(900)),
        );
        Ok(json!({"url":url,"userCode":text(&answer,"user_code")?,"waiting":true}))
    }

    pub async fn poll(&mut self) -> Result<Value> {
        if self.pending.is_none()
            || self
                .deadline
                .is_none_or(|deadline| Instant::now() >= deadline)
        {
            self.pending = None;
            return Err(Error::Refused("Sign-in expired. Start again."));
        }
        if self.next.is_some_and(|next| Instant::now() < next) {
            return Ok(json!({"waiting":true}));
        }
        let answer = post("https://api.workos.com/user_management/authenticate", &json!({"client_id":CLIENT,
            "grant_type":"urn:ietf:params:oauth:grant-type:device_code","device_code":self.pending.as_deref().map(String::as_str)}), None, true).await?;
        match answer.get("failure").and_then(Value::as_str) {
            Some(failure @ ("authorization_pending" | "slow_down")) => {
                if failure == "slow_down" {
                    self.interval = (self.interval + 5).min(60);
                }
                self.next = Some(Instant::now() + Duration::from_secs(self.interval));
                Ok(json!({"waiting":true}))
            }
            Some(_) => {
                self.pending = None;
                Err(Error::Refused("Sign-in was not completed. Start again."))
            }
            None => {
                self.pending = None;
                self.access = Some(Zeroizing::new(text(&answer, "access_token")?.into()));
                Ok(
                    json!({"signedIn":true,"email":answer["user"]["email"].as_str().unwrap_or("Signed-in Anima account")}),
                )
            }
        }
    }
}

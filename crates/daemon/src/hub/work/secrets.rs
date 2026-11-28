//! Read one work input in a zeroized buffer outside the journal.
use agent_threads::managed::workspace::CheckoutRequest;
use anima_applications_wire::{MAX_COMPOSE_BYTES, MAX_ENVIRONMENT_BYTES};
use chrono::{DateTime, Utc};
use daemon_core::{
    source_jobs::{MAX_CHECKOUT_TOKEN_BYTES, valid_checkout_token},
    wire::{COMPOSE_FILE, DaemonToken, ENVIRONMENT_FILE, Held, REPOSITORY_TOKEN, SecretRequest},
};
use serde::Deserialize;
use zeroize::Zeroizing;

use crate::hub::{
    Call, HubClient,
    answer::{Answer, Cause},
    json,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ReleasedSecret {
    #[serde(deserialize_with = "secret")]
    value: Zeroizing<String>,
    expires_at: DateTime<Utc>,
}

impl ReleasedSecret {
    pub(crate) fn live(&self) -> bool {
        self.expires_at > Utc::now()
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        self.value.as_bytes()
    }

    pub(crate) const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
}

fn secret<'de, D: serde::Deserializer<'de>>(input: D) -> Result<Zeroizing<String>, D::Error> {
    String::deserialize(input).map(Zeroizing::new)
}

impl HubClient {
    pub(crate) async fn workspace_checkout_token(
        &self,
        token: &DaemonToken,
        request: &CheckoutRequest,
    ) -> Option<ReleasedSecret> {
        let call = Call {
            route: "daemon/v3/workspaces/checkout",
            bearer: Some(token),
            body: json(request),
            response_limit: crate::hub::body::LIMIT,
            ok: &[200],
        };
        match self.one_secret(call, MAX_CHECKOUT_TOKEN_BYTES).await {
            Answer::Accepted(value) if valid_checkout_token(value.bytes()) => Some(value),
            _ => None,
        }
    }

    pub(crate) async fn apply_file(
        &self,
        token: &DaemonToken,
        item: Held,
        purpose: &'static str,
    ) -> Answer<ReleasedSecret> {
        let limit = match purpose {
            COMPOSE_FILE => MAX_COMPOSE_BYTES,
            ENVIRONMENT_FILE => MAX_ENVIRONMENT_BYTES,
            _ => return Answer::Failed(Cause::Unreadable),
        };
        self.work_secret(token, item, (purpose, limit)).await
    }

    pub(crate) async fn checkout_token(
        &self,
        token: &DaemonToken,
        item: Held,
    ) -> Option<ReleasedSecret> {
        match self
            .work_secret(token, item, (REPOSITORY_TOKEN, MAX_CHECKOUT_TOKEN_BYTES))
            .await
        {
            Answer::Accepted(value) if valid_checkout_token(value.bytes()) => Some(value),
            _ => None,
        }
    }

    async fn work_secret(
        &self,
        token: &DaemonToken,
        item: Held,
        (purpose, limit): (&'static str, usize),
    ) -> Answer<ReleasedSecret> {
        let call = Call {
            route: "work/v1/secret",
            bearer: Some(token),
            body: json(&SecretRequest {
                id: item.id,
                attempt: item.attempt,
                purpose: purpose.to_owned(),
            }),
            response_limit: crate::hub::body::LIMIT,
            ok: &[200],
        };
        self.one_secret(call, limit).await
    }

    async fn one_secret(&self, call: Call<'_>, limit: usize) -> Answer<ReleasedSecret> {
        // Secrets arrive only over the enrolled HTTPS connection.
        if self.local.is_some() {
            return Answer::Failed(Cause::Unreadable);
        }
        let cap = 6 * limit + 128;
        self.post_http(call, cap + 1, |_, bytes| {
            if bytes.len() > cap {
                return None;
            }
            let released: ReleasedSecret = serde_json::from_slice(bytes).ok()?;
            (released.live() && released.bytes().len() <= limit).then_some(released)
        })
        .await
    }
}

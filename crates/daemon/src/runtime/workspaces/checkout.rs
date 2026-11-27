//! The outbox already holds the start intent before a one-use checkout fetch.
use agent_threads::managed::workspace::{
    CheckoutGrant, CheckoutRequest, Command, LocalRequest, SOCKET,
};
use chrono::{DateTime, Utc};
use daemon_core::wire::DaemonToken;
use serde_json::Value;
use uuid::Uuid;

use super::{HubTarget, owner_socket};

pub(super) enum Outcome {
    Refused,
    Delivered(Option<Value>),
}

pub(super) async fn deliver(
    (target, token): (&HubTarget, &DaemonToken),
    command: Command,
    valid_until: DateTime<Utc>,
    repository_id: Uuid,
) -> Outcome {
    let request = CheckoutRequest {
        request: command.request,
        repository_id,
    };
    let Some(credential) = target.hub.workspace_checkout_token(token, &request).await else {
        return Outcome::Refused;
    };
    if Utc::now() >= valid_until || !credential.live() {
        return Outcome::Refused;
    }
    let local = LocalRequest::DeliverPrivate {
        command,
        valid_until,
        checkout: CheckoutGrant {
            repository_id,
            expires_at: credential.expires_at(),
        },
    };
    Outcome::Delivered(
        owner_socket::call_input(SOCKET, &local, (4096, 8192), Some(credential.bytes())).await,
    )
}

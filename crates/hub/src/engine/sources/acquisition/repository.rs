//! Issue and revoke a GitHub credential restricted to a single checkout.

use chrono::{DateTime, TimeDelta, Utc};
use provider_github::{CheckoutScope, DECLARATION};
use ring::rand::SecureRandom;
use source_exchange::Request;

use super::{MintFailure, Transport, app_key::AppKey, installation};
use crate::{clock::Clock, fleet::credentials::Secret};

pub struct RepositoryToken {
    pub value: Secret,
    pub expires_at: DateTime<Utc>,
}

pub(in crate::engine::sources) async fn mint(
    transport: &Transport,
    key: &AppKey,
    scope: CheckoutScope,
    (clock, random): (&dyn Clock, &(dyn SecureRandom + Sync)),
) -> Result<RepositoryToken, MintFailure> {
    let body = installation(
        (transport, &DECLARATION),
        key,
        (clock.now(), random),
        Some(scope),
    )
    .await?;
    let reply = scope.read(&body).ok_or(MintFailure::Unusable)?;
    let now = clock.now();
    let expires_at = reply.expires_at.filter(|&expires_at| {
        now < expires_at
            && now
                .checked_add_signed(TimeDelta::hours(1))
                .is_some_and(|limit| expires_at <= limit)
    });
    if let Some(expires_at) = expires_at {
        return Ok(RepositoryToken {
            value: Secret::new(reply.token.as_bytes().to_vec()),
            expires_at,
        });
    }
    if revoke(transport, reply.token.as_bytes()).await.is_err() {
        tracing::warn!("GitHub did not revoke a refused checkout credential");
    }
    Err(MintFailure::Unusable)
}

pub(in crate::engine::sources) async fn revoke(
    transport: &Transport,
    token: &[u8],
) -> Result<(), MintFailure> {
    let target =
        Request::get(CheckoutScope::REVOCATION_PATH, &[]).map_err(|_| MintFailure::Unusable)?;
    let url = transport
        .url(DECLARATION.api_base, &target)
        .map_err(|_| MintFailure::Unusable)?;
    transport
        .revoke_installation(url, token)
        .await
        .map_err(super::mint_fault)
}

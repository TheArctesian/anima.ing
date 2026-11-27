//! Open a connected GitHub App key for an authorized checkout outside the Hub.
use std::num::NonZeroU64;

use provider_github::{CheckoutScope, DECLARATION};
use ring::rand::SystemRandom;

use super::{LiveFailure, Scheduling, Standing, failed, reachable, standing_of, token};
use crate::{
    engine::sources::{
        acquisition::{
            MintFailure,
            repository::{RepositoryToken, mint, revoke},
        },
        model::Account,
    },
    fleet::credentials::Secret,
    ids::SourceAccountId,
};

/// Issue a fresh token for an already authorized repository ID. The caller owns project and
/// attempt authorization. The token never enters the installation cache.
///
/// # Errors
/// The account is unavailable, its key cannot be opened, or GitHub does not confirm the scope.
pub async fn mint_repository_checkout(
    scheduling: &Scheduling,
    account: SourceAccountId,
    repository_id: NonZeroU64,
) -> Result<RepositoryToken, LiveFailure> {
    let (account, declaration) = reachable(scheduling, account).await?;
    if declaration.key != DECLARATION.key {
        return Err(LiveFailure::Unusable);
    }
    if standing_of(Some(&account)) != Standing::Ready {
        return Err(LiveFailure::KeyRefused);
    }
    let failure = |error| failed(error, scheduling.clock.now());
    let key = token::open(scheduling, &account).await.map_err(failure)?;
    let issued = mint(
        &scheduling.transport,
        &key,
        CheckoutScope::new(repository_id),
        (scheduling.clock.as_ref(), &SystemRandom::new()),
    )
    .await
    .map_err(|error| failure(token::mint_failed(error)))?;
    if let Err(error) = still_connected(scheduling, &account).await {
        if revoke_repository_checkout(&scheduling.transport, &issued.value)
            .await
            .is_err()
        {
            tracing::warn!("GitHub did not revoke a checkout credential for a changed account");
        }
        return Err(error);
    }
    Ok(issued)
}

async fn still_connected(scheduling: &Scheduling, original: &Account) -> Result<(), LiveFailure> {
    let (current, _) = reachable(scheduling, original.id).await?;
    if current.credential_id != original.credential_id
        || current.instance_key != original.instance_key
        || current.source != original.source
        || standing_of(Some(&current)) != Standing::Ready
    {
        return Err(LiveFailure::KeyRefused);
    }
    Ok(())
}

/// Revoke a repository token using the token itself, including after the account is disconnected.
///
/// # Errors
/// GitHub could not be reached or did not acknowledge revocation.
pub async fn revoke_repository_checkout(
    transport: &super::Transport,
    value: &Secret,
) -> Result<(), MintFailure> {
    revoke(transport, value.expose()).await
}

/// Stabilize the account's availability while the caller commits a checkout release.
pub(crate) async fn lock_checkout_account(
    conn: &mut sqlx::PgConnection,
    account: SourceAccountId,
) -> Result<bool, sqlx::Error> {
    let held = super::store::lock(conn, account).await?;
    Ok(held
        .as_ref()
        .is_some_and(|row| row.source == DECLARATION.key)
        && standing_of(held.as_ref()) == Standing::Ready)
}

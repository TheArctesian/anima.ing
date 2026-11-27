//! Coordinate a private build checkout across Jobs, Projects and the connected forge.
mod cleanup;
pub(crate) mod workspace;

use chrono::{DateTime, Utc};
use daemon_core::wire::{REPOSITORY_TOKEN, SecretRequest};
use sqlx::PgConnection;

use super::{
    jobs::{self, CheckoutSource, Error, checkout},
    projects,
};
pub(crate) use crate::engine::sources::RepositoryToken;
use crate::{
    engine::sources::{
        LiveFailure, Scheduling, lock_checkout_account, mint_repository_checkout,
        revoke_repository_checkout,
    },
    fleet::events::ClientAddr,
    ids::{DeviceId, JobId, RepositoryId},
};

type Attempt = (JobId, u32, DeviceId);

pub use cleanup::{revoke_due, spawn};

pub(crate) struct Ticket {
    attempt: Attempt,
    repository: RepositoryId,
    source: CheckoutSource,
}

#[derive(Clone)]
pub(crate) struct Issuer(pub(crate) Scheduling);

impl Issuer {
    pub(crate) async fn start(
        &self,
        device: DeviceId,
        request: &SecretRequest,
    ) -> Result<Option<Ticket>, Error> {
        let sources = &self.0;
        let (job, attempt) = (JobId::from(request.id), request.attempt);
        let mut tx = sources.pool.begin().await?;
        let Some(held) =
            checkout::held(&mut tx, device, (job, attempt), sources.clock.as_ref()).await?
        else {
            return Ok(None);
        };
        let key = (job, attempt, device);
        if !source_matches(&mut tx, &held).await? || !checkout::reserve(&mut tx, key).await? {
            return Ok(None);
        }
        tx.commit().await?;
        Ok(Some(Ticket {
            attempt: key,
            repository: held.repository,
            source: held.source,
        }))
    }

    pub(crate) async fn mint(&self, ticket: &Ticket) -> Result<RepositoryToken, LiveFailure> {
        mint_repository_checkout(
            &self.0,
            ticket.source.account,
            ticket.source.forge_repository,
        )
        .await
    }

    pub(crate) async fn finish(
        &self,
        ticket: Ticket,
        token: &RepositoryToken,
        client: Option<ClientAddr>,
    ) -> Result<Option<DateTime<Utc>>, Error> {
        let sources = &self.0;
        let (job, attempt, device) = ticket.attempt;
        let context = checkout::seal_context(job, attempt, device);
        let sealed = sources
            .vault
            .seal(&token.value, context.as_bytes())
            .map_err(|_| Error::Unreadable)?;
        let mut tx = sources.pool.begin().await?;
        let released = match client {
            Some(client) => {
                commit_release(&mut tx, sources, (&ticket, token.expires_at), client).await?
            }
            None => None,
        };
        let recorded = released.map(|(_, time)| time);
        let sealed = sealed.to_bytes();
        checkout::save(
            &mut tx,
            ticket.attempt,
            (&sealed, token.expires_at, recorded),
        )
        .await?;
        if released.is_some_and(|(expiry, _)| sources.clock.now() >= expiry) {
            tx.rollback().await?;
            let mut revoke = sources.pool.begin().await?;
            checkout::save(
                &mut revoke,
                ticket.attempt,
                (&sealed, token.expires_at, None),
            )
            .await?;
            revoke.commit().await?;
            return Ok(None);
        }
        tx.commit().await?;
        Ok(released.map(|(expiry, _)| expiry))
    }

    pub(crate) async fn revoke_unrecorded(&self, token: &RepositoryToken) {
        if revoke_repository_checkout(&self.0.transport, &token.value)
            .await
            .is_err()
        {
            tracing::warn!("a checkout credential could not be revoked after a storage failure");
        }
    }
}

async fn commit_release(
    conn: &mut PgConnection,
    sources: &Scheduling,
    (ticket, expires_at): (&Ticket, DateTime<Utc>),
    client: ClientAddr,
) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>, Error> {
    let (job, attempt, device) = ticket.attempt;
    let Some(held) = checkout::held(conn, device, (job, attempt), sources.clock.as_ref()).await?
    else {
        return Ok(None);
    };
    if held.repository != ticket.repository
        || held.source != ticket.source
        || !source_matches(conn, &held).await?
    {
        return Ok(None);
    }
    let now = sources.clock.now();
    let expires = held.deadline.min(expires_at);
    if now >= expires {
        return Ok(None);
    }
    checkout::secret_released(conn, &held.job, (REPOSITORY_TOKEN, device), (client, now)).await?;
    Ok(Some((expires, now)))
}

async fn source_matches(conn: &mut PgConnection, held: &checkout::Held) -> Result<bool, Error> {
    let (Some(project), Some(repository)) =
        (held.job.asked.project, held.job.asked.repository.as_ref())
    else {
        return Ok(false);
    };
    let name = format!("{}/{}", repository.owner, repository.name);
    Ok(repository.forge == "github"
        && lock_checkout_account(conn, held.source.account).await?
        && projects::locked_checkout_matches(
            conn,
            (held.repository, project),
            (held.source.account, held.source.forge_repository),
            &name,
        )
        .await?)
}

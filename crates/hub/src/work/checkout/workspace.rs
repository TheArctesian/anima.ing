//! Repository authorization for checkout in a human-owned workspace.
mod releases;

use std::num::NonZeroU64;

use agent_threads::managed::workspace::CheckoutRequest;
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgConnection;
use uuid::Uuid;

use super::{Error, Issuer, projects};
use crate::{
    engine::sources::{
        LiveFailure, RepositoryToken, lock_checkout_account, mint_repository_checkout,
    },
    fleet::events::{self, AuditAction, AuditTarget, ClientAddr, NewAudit, Operator},
    ids::{DeviceId, ProjectId, RepositoryId, SourceAccountId},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    pub repository: RepositoryId,
    project: ProjectId,
    account: SourceAccountId,
    external_id: NonZeroU64,
    name: String,
}

pub(crate) async fn bind(
    conn: &mut PgConnection,
    source: &agent_threads::managed::workspace::Source,
) -> Result<Option<Binding>, sqlx::Error> {
    let Some(repository) = source.repository_id.map(RepositoryId::from) else {
        return Ok(None);
    };
    let name = format!("{}/{}", source.owner, source.repository);
    let Some(identity) = projects::checkout_identity(conn, repository, &name).await? else {
        return Ok(None);
    };
    let Ok(external_id) = identity.external_id.parse() else {
        return Ok(None);
    };
    let binding = Binding {
        repository,
        project: identity.project,
        account: identity.account,
        external_id,
        name,
    };
    Ok(still_bound(conn, &binding).await?.then_some(binding))
}

async fn still_bound(conn: &mut PgConnection, binding: &Binding) -> Result<bool, sqlx::Error> {
    Ok(lock_checkout_account(conn, binding.account).await?
        && projects::locked_checkout_matches(
            conn,
            (binding.repository, binding.project),
            (binding.account, binding.external_id),
            &binding.name,
        )
        .await?)
}

#[derive(PartialEq, Eq, sqlx::FromRow)]
struct Held {
    #[sqlx(json)]
    binding: Binding,
    workspace: Uuid,
    request_ends: DateTime<Utc>,
    offer_ends: DateTime<Utc>,
}

pub(crate) struct Ticket {
    request: Uuid,
    device: DeviceId,
    held: Held,
}

impl Ticket {
    pub(crate) fn live(&self, now: DateTime<Utc>) -> bool {
        now < self.held.offer_ends
    }
}

pub(crate) async fn reserve(
    conn: &mut PgConnection,
    device: DeviceId,
    request: &CheckoutRequest,
    now: DateTime<Utc>,
) -> Result<Option<Ticket>, Error> {
    let Some(held) = held(conn, device, request.request, now).await? else {
        return Ok(None);
    };
    if held.binding.repository != RepositoryId::from(request.repository_id)
        || !still_bound(conn, &held.binding).await?
        || !releases::reserve(conn, request.request, device, now).await?
    {
        return Ok(None);
    }
    Ok(Some(Ticket {
        request: request.request,
        device,
        held,
    }))
}

async fn held(
    conn: &mut PgConnection,
    device: DeviceId,
    request: Uuid,
    now: DateTime<Utc>,
) -> Result<Option<Held>, sqlx::Error> {
    sqlx::query_as(
        "SELECT r.checkout_binding AS binding,r.workspace,r.expires_at AS request_ends,
                r.checkout_offer_until AS offer_ends
         FROM agent_threads.workspace_requests r
         JOIN agent_threads.workspace_owners o USING (device_id,workspace)
         JOIN agent_threads.workspace_nodes n ON n.device_id=r.device_id
         WHERE r.request=$1 AND r.device_id=$2 AND r.state='offered' AND r.expires_at>$3
           AND r.checkout_offer_until>$3 AND r.payload->'action'->>'kind'='start'
           AND r.payload #> '{action,generation}'='1'::jsonb
           AND NOT o.deleted AND o.principal=r.principal AND o.created_at<=r.created_at
           AND n.ready AND n.private_checkout AND n.seen_at>$3-interval '30 seconds'
           AND NOT EXISTS (
               SELECT 1 FROM agent_threads.workspace_handoffs h
               WHERE h.workspace=r.workspace AND h.finished_at IS NULL
           )
         FOR UPDATE OF n,r FOR SHARE OF o",
    )
    .bind(request)
    .bind(device)
    .bind(now)
    .fetch_optional(conn)
    .await
}

impl Issuer {
    pub(crate) async fn mint_workspace(
        &self,
        ticket: &Ticket,
    ) -> Result<RepositoryToken, LiveFailure> {
        let binding = &ticket.held.binding;
        mint_repository_checkout(&self.0, binding.account, binding.external_id).await
    }

    pub(crate) async fn retain_workspace_token(
        &self,
        (ticket, token): (&Ticket, &RepositoryToken),
    ) -> Result<(), Error> {
        let context = releases::context(ticket.request, ticket.device);
        let sealed = self
            .0
            .vault
            .seal(&token.value, context.as_bytes())
            .map_err(|_| Error::Unreadable)?;
        let decision_ends = token
            .expires_at
            .min(self.0.clock.now() + TimeDelta::seconds(15));
        releases::retain(
            &self.0.pool,
            (ticket.request, ticket.device),
            (&sealed.to_bytes(), token.expires_at, decision_ends),
        )
        .await
    }

    pub(crate) async fn settle_workspace(
        &self,
        conn: &mut PgConnection,
        (ticket, token): (&Ticket, &RepositoryToken),
        allowed: Option<(ClientAddr, DateTime<Utc>)>,
    ) -> Result<Option<DateTime<Utc>>, Error> {
        let expiry = if let Some((client, enrolment_ends)) = allowed {
            self.release_deadline(conn, ticket, (client, enrolment_ends.min(token.expires_at)))
                .await?
        } else {
            None
        };
        releases::decide(
            conn,
            (ticket.request, ticket.device),
            (expiry, self.0.clock.now()),
        )
        .await?;
        Ok(expiry)
    }

    async fn release_deadline(
        &self,
        conn: &mut PgConnection,
        ticket: &Ticket,
        (client, bound): (ClientAddr, DateTime<Utc>),
    ) -> Result<Option<DateTime<Utc>>, Error> {
        let Some(current) = held(conn, ticket.device, ticket.request, self.0.clock.now()).await?
        else {
            return Ok(None);
        };
        if current != ticket.held || !still_bound(conn, &current.binding).await? {
            return Ok(None);
        }
        let now = self.0.clock.now();
        let expiry = bound
            .min(current.request_ends)
            .min(now + TimeDelta::seconds(85));
        if now >= expiry || now >= current.offer_ends {
            return Ok(None);
        }
        events::audit(
            conn,
            now,
            NewAudit {
                operator: Operator::hub(),
                action: AuditAction::Fetch,
                target: AuditTarget::Device(ticket.device),
                client,
                change: json!({
                    "purpose":"workspace_checkout",
                    "workspaceRequest":ticket.request,
                    "workspace":current.workspace,
                    "repository":current.binding.repository
                }),
            },
        )
        .await?;
        Ok(Some(expiry))
    }
}

pub(super) use releases::revoke_due;

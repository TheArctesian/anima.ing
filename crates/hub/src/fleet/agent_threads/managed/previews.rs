//! Human-issued preview grants, independent of browser URLs and node stream transport.
use super::{Error, Result};
use crate::ids::DeviceId;
use agent_threads::managed::preview::{Binding, Open};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct Preview {
    endpoint: Uuid,
    grant_id: Uuid,
    device_id: DeviceId,
    workspace: Uuid,
    generation: i64,
    principal: Uuid,
    port: i32,
    expires_at: DateTime<Utc>,
    revoked: bool,
}
impl Preview {
    fn view(&self) -> Value {
        json!({"endpoint":self.endpoint,"device":self.device_id,"workspace":self.workspace,
            "generation":self.generation,"port":self.port,"expiresAt":self.expires_at,"revoked":self.revoked})
    }
}

pub(super) struct Scope {
    pub device: DeviceId,
    pub workspace: Uuid,
    pub principal: Uuid,
}

pub(super) struct Request {
    pub scope: Scope,
    pub endpoint: Uuid,
    pub generation: u64,
    pub port: u16,
}

impl Request {
    fn matches(&self, row: &Preview) -> bool {
        row.device_id == self.scope.device
            && row.workspace == self.scope.workspace
            && i64::try_from(self.generation) == Ok(row.generation)
            && row.principal == self.scope.principal
            && row.port == i32::from(self.port)
    }
}

pub(super) async fn issue(
    tx: &mut PgConnection,
    request: &Request,
    now: DateTime<Utc>,
) -> Result<Value> {
    ready(tx, request, now).await?;
    if let Some(held) =
        sqlx::query_as::<_, Preview>("SELECT * FROM agent_threads.previews WHERE endpoint=$1")
            .bind(request.endpoint)
            .fetch_optional(&mut *tx)
            .await?
    {
        return request
            .matches(&held)
            .then(|| held.view())
            .ok_or(Error::Refused(
                "This preview identity belongs to another request.",
            ));
    }
    capacity(tx, &request.scope, now).await?;
    let inserted = sqlx::query(
        "INSERT INTO agent_threads.previews
        (endpoint,grant_id,device_id,workspace,generation,principal,port,created_at,expires_at)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (endpoint) DO NOTHING",
    )
    .bind(request.endpoint)
    .bind(Uuid::now_v7())
    .bind(request.scope.device)
    .bind(request.scope.workspace)
    .bind(i64::try_from(request.generation)?)
    .bind(request.scope.principal)
    .bind(i32::from(request.port))
    .bind(now)
    .bind(now + TimeDelta::minutes(10))
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let held =
        sqlx::query_as::<_, Preview>("SELECT * FROM agent_threads.previews WHERE endpoint=$1")
            .bind(request.endpoint)
            .fetch_one(tx)
            .await?;
    if inserted == 0 && !request.matches(&held) {
        return Err(Error::Refused(
            "This preview identity belongs to another request.",
        ));
    }
    Ok(held.view())
}

async fn ready(tx: &mut PgConnection, request: &Request, now: DateTime<Utc>) -> Result<()> {
    if request.endpoint.is_nil()
        || request.scope.workspace.is_nil()
        || request.scope.principal.is_nil()
        || request.port == 0
        || !(1..=9_007_199_254_740_991).contains(&request.generation)
    {
        return Err(Error::Refused("Invalid preview identity."));
    }
    let ready = sqlx::query_scalar::<_, bool>(
        "SELECT w.enabled AND w.ready AND w.generation=$3 AND w.seen_at >= $4
         FROM agent_threads.workspaces w
         LEFT JOIN agent_threads.workspace_owners o USING (device_id, workspace)
         WHERE w.device_id=$1 AND w.workspace=$2
           AND (o.principal IS NULL OR (o.principal=$5 AND NOT o.deleted))
         FOR UPDATE OF w",
    )
    .bind(request.scope.device)
    .bind(request.scope.workspace)
    .bind(i64::try_from(request.generation)?)
    .bind(now - TimeDelta::seconds(30))
    .bind(request.scope.principal)
    .fetch_optional(tx)
    .await?;
    if ready != Some(true) {
        return Err(Error::Refused(
            "Enable a connected workspace before opening a preview.",
        ));
    }
    Ok(())
}

async fn capacity(tx: &mut PgConnection, scope: &Scope, now: DateTime<Utc>) -> Result<()> {
    let (total, active): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE NOT revoked AND expires_at > $3)
        FROM agent_threads.previews WHERE device_id=$1 AND workspace=$2",
    )
    .bind(scope.device)
    .bind(scope.workspace)
    .bind(now)
    .fetch_one(tx)
    .await?;
    if total >= 4096 || active >= 16 {
        return Err(Error::Refused(
            "This workspace has reached its preview limit.",
        ));
    }
    Ok(())
}

pub(super) async fn list(pool: &PgPool, scope: &Scope, now: DateTime<Utc>) -> Result<Vec<Value>> {
    let rows = sqlx::query_as::<_, Preview>("SELECT p.* FROM agent_threads.previews p
        LEFT JOIN agent_threads.workspace_owners o USING (device_id, workspace)
        WHERE p.device_id=$1 AND p.workspace=$2 AND p.principal=$3 AND p.expires_at > $4 AND NOT p.revoked
          AND (o.principal IS NULL OR (o.principal=p.principal AND NOT o.deleted))
        ORDER BY p.created_at DESC LIMIT 16")
        .bind(scope.device).bind(scope.workspace).bind(scope.principal).bind(now).fetch_all(pool).await?;
    Ok(rows.iter().map(Preview::view).collect())
}

pub(super) async fn revoke(
    tx: &mut PgConnection,
    endpoint: Uuid,
    principal: Uuid,
) -> Result<DeviceId> {
    sqlx::query_scalar("UPDATE agent_threads.previews SET revoked=true WHERE endpoint=$1 AND principal=$2 RETURNING device_id")
        .bind(endpoint).bind(principal).fetch_optional(tx).await?.ok_or(Error::Refused("This preview is unavailable."))
}

/// The HTTP owner must call this for each connection and monitor it while streaming.
pub(in crate::fleet) async fn authorized(
    pool: &PgPool,
    endpoint: Uuid,
    principal: Uuid,
    now: DateTime<Utc>,
) -> Result<(DeviceId, Open)> {
    let row = sqlx::query_as::<_, Preview>(
        "SELECT p.* FROM agent_threads.previews p
        JOIN agent_threads.workspaces w ON w.device_id=p.device_id AND w.workspace=p.workspace
        LEFT JOIN agent_threads.workspace_owners o ON o.device_id=p.device_id AND o.workspace=p.workspace
        WHERE p.endpoint=$1 AND p.principal=$2 AND NOT p.revoked AND p.expires_at > $3
          AND w.enabled AND w.ready AND w.generation=p.generation AND w.seen_at >= $4
          AND (o.principal IS NULL OR (o.principal=p.principal AND NOT o.deleted))",
    )
    .bind(endpoint)
    .bind(principal)
    .bind(now)
    .bind(now - TimeDelta::seconds(30))
    .fetch_optional(pool)
    .await?
    .ok_or(Error::Refused(
        "This preview is unavailable, expired, or its workspace ended.",
    ))?;
    Ok((
        row.device_id,
        Open {
            binding: Binding {
                endpoint: row.endpoint,
                principal: row.principal,
                grant: row.grant_id,
                workspace: row.workspace,
                generation: u64::try_from(row.generation)?,
                port: u16::try_from(row.port)?,
                expires_at: u64::try_from(row.expires_at.timestamp())?,
            },
            valid_until: now + TimeDelta::seconds(15),
        },
    ))
}

#[cfg(test)]
mod tests;

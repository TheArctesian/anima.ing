use std::sync::Arc;

use async_graphql::{Context, ID, SimpleObject};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    api_errors::{internal, schema_dependency},
    clock::Clock,
    fleet::{
        access::{ClientAddr, Error, person_only, store::remote_sessions as store},
        events::{self, AuditAction, AuditTarget, NewAudit},
    },
};

#[derive(SimpleObject)]
pub struct CommandLineSession {
    id: ID,
    computer: String,
    created_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    linked: bool,
    current: bool,
}

pub(in crate::fleet::access) async fn list(
    ctx: &Context<'_>,
    offset: i32,
) -> async_graphql::Result<Vec<CommandLineSession>> {
    if offset < 0 {
        return Err(Error::Validation {
            field: "offset",
            message: "The offset cannot be negative.",
        }
        .into_graphql());
    }
    let identity = person_only(ctx)?;
    let operator = identity
        .operator()
        .ok_or_else(|| Error::Unauthenticated.into_graphql())?;
    let db = schema_dependency::<PgPool>(ctx)?;
    let mut conn = db.acquire().await.map_err(internal)?;
    let rows = store::list(&mut conn, operator.id, offset)
        .await
        .map_err(internal)?;
    Ok(rows
        .into_iter()
        .map(|row| CommandLineSession {
            id: ID(row.id.to_string()),
            computer: row.computer,
            created_at: row.created_at,
            last_used_at: row.last_used_at,
            expires_at: row.expires_at,
            linked: row.linked,
            current: row.id == identity.token(),
        })
        .collect())
}

pub(in crate::fleet::access) async fn revoke(
    ctx: &Context<'_>,
    id: ID,
) -> async_graphql::Result<bool> {
    let identity = person_only(ctx)?;
    let operator = identity
        .operator()
        .ok_or_else(|| Error::Unauthenticated.into_graphql())?;
    let id: Uuid = id.parse().map_err(|_| {
        Error::Validation {
            field: "id",
            message: "The session identifier is not valid.",
        }
        .into_graphql()
    })?;
    let now = schema_dependency::<Arc<dyn Clock>>(ctx)?.now();
    let client = *schema_dependency::<ClientAddr>(ctx)?;
    let db = schema_dependency::<PgPool>(ctx)?;
    let mut tx = db.begin().await.map_err(internal)?;
    let removed = store::revoke(&mut tx, operator.id, id)
        .await
        .map_err(internal)?;
    if removed {
        events::audit(
            &mut tx,
            now,
            NewAudit {
                target: AuditTarget::Operator(operator.id),
                operator: operator.clone(),
                action: AuditAction::Logout,
                client,
                change: serde_json::json!({"session": id, "tokenKind": "command_line"}),
            },
        )
        .await
        .map_err(internal)?;
    }
    tx.commit().await.map_err(internal)?;
    Ok(removed)
}

/// No expiry: an account-approved remote session ends only by revocation (OD-E32).
pub struct RemoteAuthPayload {
    token: super::super::tokens::Token,
    account: super::super::store::Account,
}

#[async_graphql::Object]
impl RemoteAuthPayload {
    async fn token(&self) -> &str {
        self.token.expose()
    }
    async fn expires_at(&self) -> Option<DateTime<Utc>> {
        None
    }
    async fn operator(&self) -> super::super::graphql::OperatorProfile {
        (&self.account).into()
    }
}

/// Issues only after the link domain checks a cloud-authorized, device-bound, single-use claim.
///
/// # Errors
/// A database or random-source failure, with no credential in the error.
pub async fn sign_in_remote(
    db: &PgPool,
    device: super::hosted::HostedDevice<'_>,
) -> async_graphql::Result<RemoteAuthPayload> {
    issue_remote(db, device).await.map_err(Error::into_graphql)
}

async fn issue_remote(
    db: &PgPool,
    device: super::hosted::HostedDevice<'_>,
) -> Result<RemoteAuthPayload, Error> {
    use crate::fleet::access::{Password, hash_password, store as accounts, tokens::Token};
    use crate::fleet::events::{Actor, ActorKind, Operator, TokenKind};
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    let password = Password::new(URL_SAFE_NO_PAD.encode(super::random::<32>()?));
    let password_hash = hash_password(password).await?;
    let token = Token::from_random(super::random()?);
    let mut tx = db.begin().await?;
    let account = accounts::hosted_operator(&mut tx, &password_hash, device.at).await?;
    let new = store::NewRemote {
        operator: account.id,
        hash: token.hash(),
        computer: device.name,
    };
    let id = store::insert(&mut tx, &new, device.at).await?;
    let operator = Operator {
        id: account.id,
        name: account.username.clone(),
        actor: Actor::request(ActorKind::User, TokenKind::CommandLine, None),
    };
    events::audit(&mut tx, device.at, NewAudit {
        target: AuditTarget::Operator(operator.id), operator, action: AuditAction::Login,
        client: device.client, change: serde_json::json!({"session": id, "computer": device.name, "through": "account", "expiresAt": null}),
    }).await?;
    tx.commit().await?;
    Ok(RemoteAuthPayload { token, account })
}

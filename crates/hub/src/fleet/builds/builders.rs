//! BLD-021 to BLD-025, D-1.

use std::net::{IpAddr, Ipv4Addr};

use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{
    Error, hidden_text,
    model::{Builder, FieldError, Missing, Platform},
    queue, store,
};
use crate::{
    fleet::{
        access::{self, IssuedToken},
        events::{self, AuditAction, AuditTarget, ClientAddr, NewAudit, Operator},
    },
    ids::{BuilderId, OperatorId, TokenId},
};

const MAX_NAME_CHARS: usize = 64;

const WRONG_ADDRESS: &str = "This builder must call from the address it enrolled from.";

pub(super) const NOT_ENROLLED: &str = "This builder is not enrolled.";

#[derive(Clone, Debug)]
pub struct BuilderInput {
    pub name: String,
    pub platform: Platform,
    /// The VPN address the request came from; the builder must call from it (BLD-024).
    pub client: IpAddr,
}

/// No `Debug`: it holds the token (LOG-007).
pub struct Enrolment {
    pub builder: Builder,
    pub token: IssuedToken,
}

impl Builder {
    #[must_use]
    pub fn online_at(&self, now: DateTime<Utc>) -> bool {
        queue::online(self.last_seen_at, now)
    }
}

/// Hidden characters are refused because they could make two names look alike.
pub(super) fn name(text: &str) -> Result<String, FieldError> {
    let text = text.trim();
    let count = text.chars().count();
    if count == 0
        || count > MAX_NAME_CHARS
        || text
            .chars()
            .any(|c| c.is_control() || hidden_text::is_hidden(c))
    {
        return Err(FieldError::new(
            "name",
            "Use 1 to 64 characters with no control or hidden characters.",
        ));
    }
    Ok(text.to_owned())
}

/// The caller commits the enrolment with its audit entry (BLD-021, BLD-022).
///
/// # Errors
///
/// `Validation` (`name`) for a bad name; `Access` if the token cannot be issued.
pub async fn enrol(
    conn: &mut PgConnection,
    operator: OperatorId,
    input: &BuilderInput,
    now: DateTime<Utc>,
) -> Result<Enrolment, Error> {
    let name = name(&input.name).map_err(|e| Error::Validation(vec![e]))?;
    let token = access::issue_builder_token(conn, operator, now)
        .await
        .map_err(Error::Access)?;
    let builder = Builder {
        id: BuilderId::from(Uuid::now_v7()),
        name,
        platform: input.platform,
        client: input.client,
        enrolled_at: now,
        last_seen_at: None,
    };
    store::insert_builder(conn, &builder, token.id).await?;
    Ok(Enrolment { builder, token })
}

/// The builder stops working at once (BLD-025); a held job returns to the queue when its lease ends.
///
/// # Errors
///
/// `NotFound` when there is no such builder; `Access` if the token cannot be revoked.
pub async fn revoke(conn: &mut PgConnection, id: BuilderId) -> Result<Builder, Error> {
    let (builder, token) = store::delete_builder(conn, id)
        .await?
        .ok_or(Error::NotFound(Missing::Builder(id)))?;
    access::revoke_builder_token(conn, token)
        .await
        .map_err(Error::Access)?;
    Ok(builder)
}

/// Every builder, in enrolment order (BLD-025).
///
/// # Errors
///
/// `Database` when the query fails.
pub async fn builders(db: &PgPool) -> Result<Vec<Builder>, Error> {
    store::builders(db).await
}

/// Only from the address the builder enrolled from (BLD-024); records the call at `now`.
///
/// # Errors
///
/// `Forbidden` when the token has no builder or the address differs; `Database` otherwise.
pub async fn builder_for(
    db: &PgPool,
    token: TokenId,
    client: Option<IpAddr>,
    now: DateTime<Utc>,
) -> Result<Builder, Error> {
    let builder = store::builder_by_token(db, token)
        .await?
        .ok_or(Error::Forbidden(NOT_ENROLLED))?;
    if client != Some(builder.client) {
        return Err(Error::Forbidden(WRONG_ADDRESS));
    }
    store::touch(db, builder.id, now).await?;
    Ok(Builder {
        last_seen_at: Some(now),
        ..builder
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    CredentialsChanged,
    TokenExpired,
}

impl Removal {
    const fn reason(self) -> &'static str {
        match self {
            Self::CredentialsChanged => "credentials changed",
            Self::TokenExpired => "token expired",
        }
    }
}

/// Audits each builder of `before` that is gone (BLD-025, LOG-004); read `before` just ahead of the deletion.
///
/// # Errors
///
/// `Database` or `Events` when the builders cannot be read or an entry cannot be written.
pub async fn audit_removed(
    db: &PgPool,
    before: &[Builder],
    now: DateTime<Utc>,
    why: Removal,
) -> Result<usize, Error> {
    let left: Vec<BuilderId> = store::builders(db).await?.iter().map(|b| b.id).collect();
    let gone: Vec<&Builder> = before.iter().filter(|b| !left.contains(&b.id)).collect();
    if gone.is_empty() {
        return Ok(0);
    }
    // The hub removes them as part of a credentials change, so the entries are Hub (LOG-014).
    let hub = Operator::hub();
    let mut tx = db.begin().await?;
    for builder in &gone {
        let entry = NewAudit {
            operator: hub.clone(),
            action: AuditAction::Delete,
            target: AuditTarget::Builder(builder.id),
            client: ClientAddr(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            change: json!({
                "name": builder.name,
                "os": builder.platform.os_str(),
                "arch": builder.platform.arch_str(),
                "clientAddr": builder.client.to_string(),
                "reason": why.reason(),
            }),
        };
        events::audit(&mut tx, now, entry).await?;
    }
    tx.commit().await?;
    Ok(gone.len())
}

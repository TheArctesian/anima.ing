//! The SQL of the tokens of the kind `command_line` and of their link to the login token of the
//! app (R-383, SGN-NEW-linked). Only the hash of a token is stored (D-7).

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    fleet::access::tokens::{Scope, TokenHash, TokenKind},
    ids::OperatorId,
};

/// A command-line token to insert, linked to the app's login token `parent`.
pub(in crate::fleet::access) struct NewCommandLine<'a> {
    pub operator: OperatorId,
    pub hash: TokenHash,
    pub parent: Uuid,
    pub computer: &'a str,
    pub expires_at: DateTime<Utc>,
}

/// A command-line token that a change deleted: its computer, for the audit entry.
pub(in crate::fleet::access) struct Ended {
    pub computer: String,
}

/// Deletes the command-line token linked to `parent`, if there is one, and returns it.
pub(in crate::fleet::access) async fn delete_linked(
    conn: &mut PgConnection,
    parent: Uuid,
) -> Result<Vec<Ended>, sqlx::Error> {
    let computers: Vec<String> = sqlx::query_scalar(
        "delete from access.api_tokens where parent_id = $1 and kind = $2 returning computer",
    )
    .bind(parent)
    .bind(TokenKind::CommandLine.as_str())
    .fetch_all(conn)
    .await?;
    Ok(computers
        .into_iter()
        .map(|computer| Ended { computer })
        .collect())
}

/// Inserts the command-line token, which ends when its parent ends, and returns its id. The
/// caller deleted the one before (one for each parent: `api_tokens_one_command_line`).
pub(in crate::fleet::access) async fn insert_command_line(
    conn: &mut PgConnection,
    token: &NewCommandLine<'_>,
    now: DateTime<Utc>,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::now_v7();
    sqlx::query(
        "insert into access.api_tokens \
         (id, operator_id, token_hash, kind, scope, expires_at, created_at, parent_id, computer) \
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(id)
    .bind(token.operator)
    .bind(&token.hash.0[..])
    .bind(TokenKind::CommandLine.as_str())
    .bind(Scope::Write.as_str())
    .bind(token.expires_at)
    .bind(now)
    .bind(token.parent)
    .bind(token.computer)
    .execute(conn)
    .await?;
    Ok(id)
}

/// Records that the login token `id` is the sign-in of the device `device` of a hosted hub.
pub(in crate::fleet::access) async fn mark_link_device(
    conn: &mut PgConnection,
    id: Uuid,
    device: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("update access.api_tokens set link_device = $2 where id = $1 and kind = $3")
        .bind(id)
        .bind(device)
        .bind(TokenKind::Login.as_str())
        .execute(conn)
        .await?;
    Ok(())
}

/// The earlier login tokens of the device `device` of `operator`, other than `current`, locked
/// until the end of the transaction: the sign-ins that a new sign-in of that device replaces.
pub(in crate::fleet::access) async fn earlier_of_device(
    conn: &mut PgConnection,
    operator: OperatorId,
    device: &str,
    current: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "select id from access.api_tokens where operator_id = $1 and link_device = $2 \
         and kind = $3 and id <> $4 order by created_at desc, id desc for update",
    )
    .bind(operator)
    .bind(device)
    .bind(TokenKind::Login.as_str())
    .bind(current)
    .fetch_all(conn)
    .await
}

/// Moves the command-line token of `from` to the login token `to`, with the end of `to`.
/// Returns its computer, or `None` when `from` had none or `to` has one already.
pub(in crate::fleet::access) async fn move_linked(
    conn: &mut PgConnection,
    from: Uuid,
    to: Uuid,
    expires_at: DateTime<Utc>,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "update access.api_tokens set parent_id = $2, expires_at = $3 \
         where parent_id = $1 and kind = $4 \
         and not exists (select from access.api_tokens where parent_id = $2 and kind = $4) \
         returning computer",
    )
    .bind(from)
    .bind(to)
    .bind(expires_at)
    .bind(TokenKind::CommandLine.as_str())
    .fetch_optional(conn)
    .await
}

/// Serializes token issuance with revocation of its approving app or browser.
pub(in crate::fleet::access) async fn lock_sign_in(
    conn: &mut PgConnection,
    parent: Uuid,
    now: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    let found: Option<Uuid> = sqlx::query_scalar(
        "select id from access.api_tokens where id = $1 and kind in ('login', 'browser') and expires_at > $2 and (idle_seconds is null or last_used_at + idle_seconds * interval '1 second' > $2) for update",
    ).bind(parent).bind(now).fetch_optional(conn).await?;
    Ok(found.is_some())
}

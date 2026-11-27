use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::ids::OperatorId;

#[derive(FromRow)]
pub(in crate::fleet::access) struct RemoteSession {
    pub id: Uuid,
    pub computer: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub linked: bool,
}

pub(in crate::fleet::access) async fn list(
    conn: &mut PgConnection,
    operator: OperatorId,
    offset: i32,
) -> Result<Vec<RemoteSession>, sqlx::Error> {
    sqlx::query_as(
        "select id, computer, created_at, last_used_at, expires_at, parent_id is not null as linked \
         from access.api_tokens where operator_id = $1 and kind = 'command_line' \
         order by created_at desc, id desc limit 100 offset $2",
    ).bind(operator).bind(offset).fetch_all(conn).await
}

pub(in crate::fleet::access) async fn revoke(
    conn: &mut PgConnection,
    operator: OperatorId,
    id: Uuid,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query("delete from access.api_tokens where id = $1 and operator_id = $2 and kind = 'command_line'")
        .bind(id).bind(operator).execute(conn).await?;
    Ok(done.rows_affected() == 1)
}

pub(in crate::fleet::access) struct NewRemote<'a> {
    pub operator: OperatorId,
    pub hash: crate::fleet::access::tokens::TokenHash,
    pub computer: &'a str,
}

pub(in crate::fleet::access) async fn insert(
    conn: &mut PgConnection,
    token: &NewRemote<'_>,
    now: DateTime<Utc>,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::now_v7();
    sqlx::query("insert into access.api_tokens (id, operator_id, token_hash, kind, scope, expires_at, created_at, computer, last_used_at) values ($1, $2, $3, 'command_line', 'write', null, $4, $5, $4)")
        .bind(id).bind(token.operator).bind(&token.hash.0[..]).bind(now).bind(token.computer)
        .execute(conn).await?;
    Ok(id)
}

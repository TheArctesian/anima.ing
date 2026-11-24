use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::{fleet::access::tokens::TokenHash, ids::OperatorId};

pub(in crate::fleet::access) struct NewSignIn<'a> {
    pub device: TokenHash,
    pub user: &'a [u8],
    pub computer: &'a str,
    pub client: &'a str,
}

pub(in crate::fleet::access) async fn start_sign_in(
    conn: &mut PgConnection,
    request: &NewSignIn<'_>,
    now: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 34))")
        .bind(request.client)
        .execute(&mut *conn)
        .await?;
    let count: i64 = sqlx::query_scalar(
        "select count(*) from access.command_line_sign_ins where client = $1 and created_at > $2",
    )
    .bind(request.client)
    .bind(now - TimeDelta::hours(1))
    .fetch_one(&mut *conn)
    .await?;
    if count >= 10 {
        return Ok(false);
    }
    sqlx::query(
        "insert into access.command_line_sign_ins \
         (id, device_hash, user_hash, computer, client, created_at, expires_at, next_poll_at) \
         values ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(Uuid::now_v7())
    .bind(&request.device.0[..])
    .bind(request.user)
    .bind(request.computer)
    .bind(request.client)
    .bind(now)
    .bind(now + TimeDelta::minutes(10))
    .bind(now + TimeDelta::seconds(5))
    .execute(conn)
    .await?;
    Ok(true)
}

#[derive(FromRow)]
pub(in crate::fleet::access) struct PendingSignIn {
    pub id: Uuid,
    pub computer: String,
    pub client: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub next_poll_at: DateTime<Utc>,
    pub interval_seconds: i32,
    pub state: String,
    pub operator_id: Option<OperatorId>,
}

const COLUMNS: &str = "id, computer, client, created_at, expires_at, next_poll_at, interval_seconds, state, operator_id";

pub(in crate::fleet::access) async fn sign_in_by_device(
    conn: &mut PgConnection,
    hash: TokenHash,
) -> Result<Option<PendingSignIn>, sqlx::Error> {
    sqlx::query_as(&format!(
        "select {COLUMNS} from access.command_line_sign_ins where device_hash = $1 for update"
    ))
    .bind(&hash.0[..])
    .fetch_optional(conn)
    .await
}

pub(in crate::fleet::access) async fn sign_in_by_user(
    conn: &mut PgConnection,
    hash: &[u8],
) -> Result<Option<PendingSignIn>, sqlx::Error> {
    sqlx::query_as(&format!(
        "select {COLUMNS} from access.command_line_sign_ins where user_hash = $1 for update"
    ))
    .bind(hash)
    .fetch_optional(conn)
    .await
}

pub(in crate::fleet::access) async fn pace_sign_in(
    conn: &mut PgConnection,
    id: Uuid,
    interval: i32,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query("update access.command_line_sign_ins set next_poll_at = $2, interval_seconds = $3 where id = $1")
        .bind(id).bind(now + TimeDelta::seconds(i64::from(interval))).bind(interval)
        .execute(conn).await?;
    Ok(())
}

pub(in crate::fleet::access) async fn finish_sign_in(
    conn: &mut PgConnection,
    id: Uuid,
    state: &str,
    operator: Option<OperatorId>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "update access.command_line_sign_ins set state = $2, operator_id = $3 where id = $1",
    )
    .bind(id)
    .bind(state)
    .bind(operator)
    .execute(conn)
    .await?;
    Ok(())
}

pub(in crate::fleet::access) async fn insert_remote_token(
    conn: &mut PgConnection,
    pending: &PendingSignIn,
    hash: TokenHash,
    now: DateTime<Utc>,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::now_v7();
    sqlx::query(
        "insert into access.api_tokens \
         (id, operator_id, token_hash, kind, scope, expires_at, created_at, computer, last_used_at) \
         values ($1, $2, $3, 'command_line', 'write', null, $4, $5, $4)",
    ).bind(id).bind(pending.operator_id).bind(&hash.0[..]).bind(now).bind(&pending.computer)
        .execute(conn).await?;
    Ok(id)
}

pub(in crate::fleet::access) async fn code_attempt_allowed(
    conn: &mut PgConnection,
    operator: OperatorId,
    now: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    sqlx::query("insert into access.command_line_code_attempts (operator_id, window_ends_at) values ($1, $2) on conflict (operator_id) do nothing")
        .bind(operator).bind(now + TimeDelta::minutes(10)).execute(&mut *conn).await?;
    let (failures, ends): (i32, DateTime<Utc>) = sqlx::query_as("select failures, window_ends_at from access.command_line_code_attempts where operator_id = $1 for update")
        .bind(operator).fetch_one(&mut *conn).await?;
    if ends <= now {
        sqlx::query("update access.command_line_code_attempts set failures = 0, window_ends_at = $2 where operator_id = $1")
            .bind(operator).bind(now + TimeDelta::minutes(10)).execute(conn).await?;
        return Ok(true);
    }
    Ok(failures < 5)
}

pub(in crate::fleet::access) async fn wrong_code(
    conn: &mut PgConnection,
    operator: OperatorId,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query("update access.command_line_code_attempts set failures = failures + 1, window_ends_at = case when failures + 1 >= 5 then $2 else window_ends_at end where operator_id = $1")
        .bind(operator).bind(now + TimeDelta::minutes(10)).execute(conn).await?;
    Ok(())
}

/// Short-lived approval material is removed by the daily access-retention job.
pub(in crate::fleet::access) async fn prune(
    conn: &mut PgConnection,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query("delete from access.command_line_sign_ins where created_at < $1")
        .bind(now - TimeDelta::hours(1))
        .execute(&mut *conn)
        .await?;
    sqlx::query("delete from access.command_line_code_attempts where window_ends_at <= $1")
        .bind(now)
        .execute(conn)
        .await?;
    Ok(())
}

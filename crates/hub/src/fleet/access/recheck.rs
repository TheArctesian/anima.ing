//! The second factor asked again before a reveal of a sealed record (K-E5 Section 3, VLT-005).
//! It passes when the login passed a TOTP code in the last 5 minutes, or with a new code, which
//! it claims as used and records on the login. Five wrong codes delete the login. A sign-in
//! with a code records it on its new token here too.

use std::sync::Arc;

use async_graphql::Context;
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{
    Error, Identity, store,
    totp::{self, SecondFactor},
};
use crate::{
    api_errors::{internal, schema_dependency},
    clock::Clock,
    fleet::{
        credentials::Vault,
        events::{self, Bus, NewEvent, Severity, Source},
    },
};

/// How long a passed TOTP code counts for a reveal.
pub const RECHECK_WINDOW: TimeDelta = TimeDelta::minutes(5);

/// Wrong codes after which the login is deleted.
const WRONG_CODES: i16 = 5;

/// What the second factor makes of a reveal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recheck {
    /// The login passed a code in the last 5 minutes, or the code given is a new valid one.
    Passed,
    /// A new code is needed: none was given, or it is wrong, malformed or used.
    CodeRequired,
    /// The operator has no TOTP, so nothing can be asked again.
    NotEnrolled,
    /// The fifth wrong code: the login is deleted.
    SignedOut,
}

/// Asks the second factor of the request's login again, now, with `code` if the caller gave
/// one. A token that no operator signed in with never passes.
///
/// # Errors
///
/// `INTERNAL` when the operator's TOTP secret does not open, or when the check cannot be read
/// or recorded.
pub async fn recheck_totp(ctx: &Context<'_>, code: Option<&str>) -> async_graphql::Result<Recheck> {
    let hub = (
        schema_dependency::<PgPool>(ctx)?,
        schema_dependency::<Bus>(ctx)?,
        schema_dependency::<Vault>(ctx)?,
    );
    let identity = schema_dependency::<Identity>(ctx)?;
    let now = schema_dependency::<Arc<dyn Clock>>(ctx)?.now();
    recheck(hub, identity, code, now).await.map_err(internal)
}

async fn recheck(
    (db, bus, vault): (&PgPool, &Bus, &Vault),
    identity: &Identity,
    code: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Recheck, Error> {
    let Some(operator) = identity.operator() else {
        return Ok(Recheck::CodeRequired);
    };
    let token = identity.token();
    let mut tx = db.begin().await?;
    let Some(account) = store::account_by_id(&mut *tx, operator.id).await? else {
        return Ok(Recheck::CodeRequired);
    };
    if account.totp_secret_enc.is_none() {
        return Ok(Recheck::NotEnrolled);
    }
    let passed_at = store::lock_token_totp(&mut tx, token).await?;
    if passed_at.is_some_and(|at| at <= now && now - at <= RECHECK_WINDOW) {
        return Ok(Recheck::Passed);
    }
    let Some(code) = code else {
        return Ok(Recheck::CodeRequired);
    };
    let outcome = match totp::second_factor(vault, &account, Some(code), now) {
        SecondFactor::Claim(step) => {
            let sealed = account.totp_secret_enc.as_deref().unwrap_or_default();
            if store::claim_totp_step(&mut *tx, account.id, sealed, step).await? {
                store::pass_token_totp(&mut tx, token, now).await?;
                Recheck::Passed
            } else {
                wrong(&mut tx, token).await?
            }
        }
        SecondFactor::Refuse => wrong(&mut tx, token).await?,
        SecondFactor::NotEnrolled => Recheck::NotEnrolled,
        SecondFactor::Unopenable(error) => return Err(Error::TotpSecret(error)),
    };
    if outcome == Recheck::SignedOut {
        let message = format!(
            "A sign-in of operator {} was deleted after {WRONG_CODES} wrong TOTP codes for a \
             reveal.",
            operator.name
        );
        let event = NewEvent {
            severity: Severity::Warning,
            source: Source::Access,
            device: None,
            message,
            actor: operator.actor.kind(),
        };
        events::record(&mut tx, bus, now, event).await?;
    }
    tx.commit().await?;
    Ok(outcome)
}

/// Counts a wrong code of the token; the fifth deletes the token.
async fn wrong(conn: &mut PgConnection, token: Uuid) -> Result<Recheck, Error> {
    if store::fail_token_totp(&mut *conn, token).await? < WRONG_CODES {
        return Ok(Recheck::CodeRequired);
    }
    store::delete_token(conn, token).await?;
    Ok(Recheck::SignedOut)
}

/// Claims the TOTP step of the sign-in, if any, and records on the new token that it passed a
/// code at that time, so a reveal in the next 5 minutes takes no new code (K-E5 Section 3,
/// VLT-005). `false` when the step was used since it was checked.
pub(super) async fn claim_step(
    conn: &mut PgConnection,
    account: &store::Account,
    step: Option<i64>,
    (token, at): (Uuid, DateTime<Utc>),
) -> Result<bool, Error> {
    let Some(step) = step else {
        return Ok(true);
    };
    let sealed = account.totp_secret_enc.as_deref().unwrap_or_default();
    if !store::claim_totp_step(&mut *conn, account.id, sealed, step).await? {
        return Ok(false);
    }
    store::pass_token_totp(conn, token, at).await?;
    Ok(true)
}

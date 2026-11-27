//! Signing out: a `logout`, and the browser token a new sign-in replaces (SEC-002, D-3, LOG-005).
//! The sign-out of the app's login token ends its command-line token too (R-383).

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};

use super::{
    Replaced, audit_entry,
    command_line::{Change, end_linked},
};
use crate::fleet::{
    access::{ClientAddr, Error, Identity, store},
    events::{self, AuditAction},
};

/// Deletes the identity's token and its linked command-line token, and audits each sign-out
/// in one transaction.
pub(in crate::fleet::access) async fn logout(
    db: &PgPool,
    identity: &Identity,
    client: ClientAddr,
    now: DateTime<Utc>,
) -> Result<(), Error> {
    let mut tx = db.begin().await?;
    let operator = identity.operator().cloned().ok_or(Error::Unauthenticated)?;
    let change = Change {
        operator: &operator,
        client,
        at: now,
    };
    end_linked(&mut tx, identity.token(), &change).await?;
    if !store::delete_token(&mut tx, identity.token()).await? {
        return Err(Error::Unauthenticated);
    }
    let entry = audit_entry(operator, AuditAction::Logout, client);
    events::audit(&mut tx, now, entry).await?;
    tx.commit().await?;
    Ok(())
}

/// Deletes the replaced browser token and audits it as a sign-out of its operator, as
/// `logout` does. A token already gone (a `logout` that committed first) is left alone.
pub(super) async fn retire(
    conn: &mut PgConnection,
    replaced: Replaced,
    client: ClientAddr,
    at: DateTime<Utc>,
) -> Result<(), Error> {
    if store::delete_token(conn, replaced.token).await? {
        let entry = audit_entry(replaced.operator, AuditAction::Logout, client);
        events::audit(conn, at, entry).await?;
    }
    Ok(())
}

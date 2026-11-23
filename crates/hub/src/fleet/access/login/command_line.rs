//! One sign-in (R-383; K-E5 Section 3b, SGN-NEW-linked): the token of the kind `command_line`
//! that the desktop app asks for and gives to the `anima` program on its computer, its end with
//! the app's token, and its move to the app's new token when a device of a hosted hub signs in
//! again. Each token that ends or moves has its audit entry; no entry holds a token (LOG-007).

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{SignedIn, random};
use crate::fleet::{
    access::{
        ClientAddr, Error, Identity, Operator,
        store::{self, NewCommandLine},
        tokens::{TOKEN_BYTES, Token, TokenKind},
    },
    events::{self, ActorKind, AuditAction, AuditTarget, NewAudit},
};

/// The most characters of the name of a computer, as for a device on the link.
const COMPUTER_CHARS: usize = 64;

/// What `issueCommandLineToken` says of a name that it refuses.
const COMPUTER_RULE: &str =
    "Give the name of this computer: 1 to 64 characters, no control characters.";

/// Who changes the tokens, from which address and when: what each audit entry records.
pub(in crate::fleet::access) struct Change<'a> {
    pub operator: &'a Operator,
    pub client: ClientAddr,
    pub at: DateTime<Utc>,
}

impl Change<'_> {
    async fn audit(
        &self,
        conn: &mut PgConnection,
        action: AuditAction,
        change: serde_json::Value,
    ) -> Result<(), Error> {
        let entry = NewAudit {
            target: AuditTarget::Operator(self.operator.id),
            operator: self.operator.clone(),
            action,
            client: self.client,
            change,
        };
        events::audit(conn, self.at, entry).await?;
        Ok(())
    }
}

/// The name of the computer, trimmed, or a refusal.
fn computer_name(name: &str) -> Result<&str, Error> {
    let name = name.trim();
    let length = name.chars().count();
    if length == 0 || length > COMPUTER_CHARS || name.chars().any(char::is_control) {
        return Err(Error::Validation {
            field: "computer",
            message: COMPUTER_RULE,
        });
    }
    Ok(name)
}

/// Gives the `anima` program on `computer` a command-line token linked to the login token of
/// this request, which must be the desktop app's, sent by the person and not an agent. The
/// token ends when the app's token ends. A token linked before to the same app token is
/// deleted, so that each app sign-in has one.
///
/// # Errors
///
/// `FORBIDDEN` for each token but a login token of a person, `VALIDATION` for the name, and a
/// hub fault when the token cannot be made or kept.
pub(in crate::fleet::access) async fn issue(
    db: &PgPool,
    identity: &Identity,
    computer: &str,
    (client, at): (ClientAddr, DateTime<Utc>),
) -> Result<SignedIn, Error> {
    let operator = identity.operator().ok_or(Error::KindRefused)?;
    if identity.kind() != TokenKind::Login || operator.actor.kind() != ActorKind::User {
        return Err(Error::KindRefused);
    }
    let computer = computer_name(computer)?;
    let token = Token::from_random(random::<TOKEN_BYTES>()?);
    let expires_at = identity.expires_at();
    let change = Change {
        operator,
        client,
        at,
    };
    let mut tx = db.begin().await?;
    let account = store::account_by_id(&mut *tx, operator.id)
        .await?
        .ok_or(Error::Unauthenticated)?;
    for ended in store::delete_linked(&mut tx, identity.token()).await? {
        let record = serde_json::json!({ "commandLine": ended.computer, "replaced": true });
        change.audit(&mut tx, AuditAction::Logout, record).await?;
    }
    let new = NewCommandLine {
        operator: operator.id,
        hash: token.hash(),
        parent: identity.token(),
        computer,
        expires_at,
    };
    store::insert_command_line(&mut tx, &new, at).await?;
    let record = serde_json::json!({ "commandLine": computer, "linkedTo": "app" });
    change.audit(&mut tx, AuditAction::Login, record).await?;
    tx.commit().await?;
    Ok(SignedIn {
        token,
        expires_at,
        account,
    })
}

/// Deletes the command-line token linked to the app's token `parent`, with an audit entry,
/// in the transaction of the app's sign-out (SGN-NEW-linked).
pub(in crate::fleet::access) async fn end_linked(
    conn: &mut PgConnection,
    parent: Uuid,
    change: &Change<'_>,
) -> Result<(), Error> {
    for ended in store::delete_linked(conn, parent).await? {
        let record = serde_json::json!({ "commandLine": ended.computer, "with": "app" });
        change.audit(conn, AuditAction::Logout, record).await?;
    }
    Ok(())
}

/// A new sign-in `current` (its token and its end) of the device `device` of a hosted hub
/// replaces the device's earlier sign-ins: the command-line token of the newest moves to
/// `current`, with its end, and each earlier login token is deleted with any other linked
/// token, each with an audit entry.
pub(in crate::fleet::access) async fn renew(
    conn: &mut PgConnection,
    device: &str,
    (to, expires_at): (Uuid, DateTime<Utc>),
    change: &Change<'_>,
) -> Result<(), Error> {
    store::mark_link_device(conn, to, device).await?;
    let earlier = store::earlier_of_device(conn, change.operator.id, device, to).await?;
    for from in earlier {
        if let Some(computer) = store::move_linked(conn, from, to, expires_at).await? {
            let record = serde_json::json!({ "commandLine": computer, "movedTo": "new sign-in" });
            change.audit(conn, AuditAction::Login, record).await?;
        }
        end_linked(conn, from, change).await?;
        if store::delete_token(conn, from).await? {
            let record = serde_json::json!({ "device": device, "replaced": true });
            change.audit(conn, AuditAction::Logout, record).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::computer_name;

    #[test]
    fn sgn_linked_the_name_of_the_computer_is_trimmed_and_checked() {
        assert_eq!(computer_name("  studio-mac ").ok(), Some("studio-mac"));
        assert!(computer_name("   ").is_err());
        assert!(computer_name(&"x".repeat(65)).is_err());
        assert!(computer_name("a\nb").is_err());
        assert_eq!(computer_name(&"é".repeat(64)).ok().map(str::len), Some(128));
    }
}

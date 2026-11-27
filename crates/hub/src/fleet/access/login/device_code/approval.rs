use async_graphql::{Context, SimpleObject};
use chrono::{DateTime, Utc};
use zeroize::Zeroizing;

use super::{Flow, store, user_hash};
use crate::fleet::{
    access::{Error, Identity, TokenKind},
    events::{self, ActorKind, AuditAction, AuditTarget, NewAudit},
};

#[derive(SimpleObject)]
pub struct CommandLineSignInDetails {
    computer: String,
    address: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

pub(in crate::fleet::access) async fn describe(
    ctx: &Context<'_>,
    user_code: String,
) -> async_graphql::Result<Option<CommandLineSignInDetails>> {
    act(ctx, user_code, None).await.map_err(Error::into_graphql)
}

pub(in crate::fleet::access) async fn approve(
    ctx: &Context<'_>,
    user_code: String,
    approve: bool,
) -> async_graphql::Result<bool> {
    act(ctx, user_code, Some(approve))
        .await
        .map(|value| value.is_some())
        .map_err(Error::into_graphql)
}

async fn act(
    ctx: &Context<'_>,
    user_code: String,
    approval: Option<bool>,
) -> Result<Option<CommandLineSignInDetails>, Error> {
    let code = Zeroizing::new(user_code);
    let flow = Flow::of(ctx)?;
    let identity = ctx.data_opt::<Identity>().ok_or(Error::Unauthenticated)?;
    let operator = identity.operator().ok_or(Error::KindRefused)?;
    if !matches!(identity.kind(), TokenKind::Login | TokenKind::Browser)
        || operator.actor.kind() != ActorKind::User
    {
        return Err(Error::KindRefused);
    }
    let mut tx = flow.db.begin().await?;
    if !crate::fleet::access::store::command_line::lock_sign_in(&mut tx, identity.token(), flow.now)
        .await?
    {
        return Err(Error::Unauthenticated);
    }
    if !store::code_attempt_allowed(&mut tx, operator.id, flow.now).await? {
        return Err(Error::Validation {
            field: "userCode",
            message: "Too many incorrect codes. Try again in ten minutes.",
        });
    }
    let pending = store::sign_in_by_user(&mut tx, &user_hash(&code))
        .await?
        .filter(|row| row.expires_at > flow.now && row.state == "waiting");
    let Some(pending) = pending else {
        store::wrong_code(&mut tx, operator.id, flow.now).await?;
        tx.commit().await?;
        return Ok(None);
    };
    if let Some(approve) = approval {
        let state = if approve { "approved" } else { "denied" };
        store::finish_sign_in(&mut tx, pending.id, state, Some(operator.id)).await?;
        events::audit(
            &mut tx,
            flow.now,
            NewAudit {
                target: AuditTarget::Operator(operator.id),
                operator: operator.clone(),
                action: AuditAction::Login,
                client: flow.client,
                change: serde_json::json!({"commandLine": pending.computer, "approval": state}),
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Some(CommandLineSignInDetails {
        computer: pending.computer,
        address: pending.client,
        created_at: pending.created_at,
        expires_at: pending.expires_at,
    }))
}

//! One named change (K-E1 Section 13.2).

use source_exchange::{Body, Change, ChangeReply, Settled};

use super::{LiveFailure, Reserve, budget::limited_until, failed, reachable, token::token};
use crate::{
    engine::sources::{
        Scheduling,
        acquisition::transport::{Content, Delivery, Fault},
    },
    ids::SourceAccountId,
};

/// A change goes out once: with no whole answer the outcome is [`Settled::Unknown`].
///
/// # Errors
/// [`LiveFailure`] only when nothing was sent.
pub async fn change(
    s: &Scheduling,
    account: SourceAccountId,
    change: &Change,
    reserve: Reserve,
) -> Result<Settled, LiveFailure> {
    let (stored, declaration) = reachable(s, account).await?;
    let changes = declaration.changes.ok_or(LiveFailure::NoChanges)?;
    let live = s.transport.live();
    if reserve == Reserve::Kept {
        live.budget(|b| b.read(account, s.clock.now()))
            .map_err(|until| LiveFailure::Limited { until })?;
    }
    let turn = live
        .pace
        .change(account, s.clock.clone())
        .await
        .map_err(|_| LiveFailure::Busy)?;
    let token = token(s, &stored, &declaration)
        .await
        .map_err(|failure| failed(failure, s.clock.now()))?;
    let delivery = s
        .transport
        .change(&declaration, change, &token)
        .await
        .map_err(|fault| match fault {
            Fault::Refused => LiveFailure::Refused,
            Fault::Network => LiveFailure::Unreachable,
        })?;
    drop(turn);
    let Delivery::Answered(answer) = delivery else {
        return Ok(Settled::Unknown);
    };
    let at = s.clock.now();
    live.budget(|b| b.note(account, answer.limits, at));
    if let Some(until) = limited_until(answer.status, answer.limits, at) {
        live.budget(|b| b.limited(account, until));
    }
    if answer.status == 401 {
        s.transport.tokens().forget(account);
    }
    let reply = ChangeReply {
        status: answer.status,
        body: match &answer.content {
            Content::Body(bytes) => Body::Read(bytes),
            Content::TooLarge => Body::TooLarge,
        },
        limit: answer.limit(),
        at,
    };
    Ok((changes.settle)(change, &reply))
}

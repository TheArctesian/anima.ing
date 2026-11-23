//! One live read: the account, the cache, the budget, the pace, the token and the send, in
//! that order (K-E1 Section 13.1).

use std::sync::Arc;

use source_exchange::{Declaration, Request};

use super::{
    Keep, LiveFailure, Reply,
    budget::limited_until,
    failed, reachable,
    replies::{Held, Key},
    token::token,
};
use crate::{
    engine::sources::{
        Scheduling,
        acquisition::transport::{Content, Fault},
        model::Account,
    },
    ids::SourceAccountId,
};

/// Reads `request` for `account`: nothing for a paused or removed account, held or not; the held
/// answer while `keep` says it is fresh; otherwise a new `GET`, held when 2xx. A passing failure
/// returns an answer held for at most an hour instead, marked with the failure.
///
/// # Errors
/// [`LiveFailure`]; a 401 drops the token (`KeyRefused`).
pub async fn ask(
    s: &Scheduling,
    account: SourceAccountId,
    request: &Request,
    keep: Keep,
) -> Result<Reply, LiveFailure> {
    let key = Key::of(account, request);
    let live = s.transport.live();
    let sent = match reachable(s, account).await {
        Ok(reached) => {
            let now = s.clock.now();
            if let Some(held) = live.replies(|r| r.fresh(&key, keep, now)) {
                return Ok(reply(held, None));
            }
            send(s, reached, request).await
        }
        Err(failure) => Err(failure),
    };
    match sent {
        Ok(answer) => Ok(settle(s, (key, keep), answer)),
        Err(failure) if failure.passes() => {
            let now = s.clock.now();
            live.replies(|r| r.stale(&key, keep, now))
                .map(|held| reply(held, Some(failure)))
                .ok_or(failure)
        }
        Err(failure) => Err(failure),
    }
}

struct Answered {
    status: u16,
    body: Option<Arc<[u8]>>,
}

async fn send(
    s: &Scheduling,
    (account, declaration): (Account, Declaration),
    request: &Request,
) -> Result<Answered, LiveFailure> {
    let id = account.id;
    let url = s
        .transport
        .url(declaration.api_base, request)
        .map_err(|_| LiveFailure::Refused)?;
    let live = s.transport.live();
    let now = s.clock.now();
    live.budget(|b| b.read(id, now))
        .map_err(|until| LiveFailure::Limited { until })?;
    let _turn = live.pace.read(id).await.map_err(|_| LiveFailure::Busy)?;
    let token = token(s, &account, &declaration)
        .await
        .map_err(|failure| failed(failure, s.clock.now()))?;
    let answer = s
        .transport
        .get(request, url, &token)
        .await
        .map_err(|fault| match fault {
            Fault::Refused => LiveFailure::Refused,
            Fault::Network => LiveFailure::Unreachable,
        })?;
    let now = s.clock.now();
    live.budget(|b| b.note(id, answer.limits, now));
    if answer.status == 401 {
        s.transport.tokens().forget(id);
        return Err(LiveFailure::KeyRefused);
    }
    if let Some(until) = limited_until(answer.status, answer.limits, now) {
        live.budget(|b| b.limited(id, until));
        return Err(LiveFailure::Limited { until });
    }
    if answer.status >= 500 {
        return Err(LiveFailure::Unusable);
    }
    Ok(Answered {
        status: answer.status,
        body: match &answer.content {
            Content::Body(bytes) => Some(Arc::from(bytes.as_slice())),
            Content::TooLarge => None,
        },
    })
}

fn settle(s: &Scheduling, (key, keep): (Key, Keep), answer: Answered) -> Reply {
    let at = s.clock.now();
    let live = s.transport.live();
    match (&answer.body, answer.status) {
        (Some(body), 200..=299) => {
            let held = Held {
                status: answer.status,
                body: body.clone(),
                at,
            };
            live.replies(|r| r.keep(key, keep, held));
        }
        _ => live.replies(|r| r.discard(&key)),
    }
    Reply {
        status: answer.status,
        body: answer.body,
        at,
        problem: None,
    }
}

fn reply(held: Held, problem: Option<LiveFailure>) -> Reply {
    Reply {
        status: held.status,
        body: Some(held.body),
        at: held.at,
        problem,
    }
}

//! Converts the code from "Create GitHub App" into the App's facts and key (PRJ-039).
//! The code is untrusted; the reply holds the App's key, and no error or log quotes it (LOG-007).
//! The POST carries no credential and no body, and the reply is wiped on drop.

use std::time::Duration;

use chrono::{DateTime, Utc};
use source_exchange::{Converted, Declaration, Registration, Request};

use super::{
    registration,
    transport::{Content, Fault, Transport},
    verdict::{Verdict, judge},
};

pub const RETRY_AFTER: Duration = Duration::from_secs(2);
const MAX_CODE: usize = 200;

/// Why no App came back. Fixed words: never the code or the reply (LOG-007).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConvertFailure {
    #[error("the source refused the code")]
    Refused,
    #[error("the source limits requests")]
    RateLimited(DateTime<Utc>),
    #[error("the source could not be reached")]
    Unreachable,
    #[error("the source's reply to the conversion could not be used")]
    Unusable,
    #[error("the source cannot make an App for the operator")]
    NotRegistrable,
}

enum Miss {
    Again(ConvertFailure),
    Final(ConvertFailure),
}

impl Miss {
    const fn failure(self) -> ConvertFailure {
        match self {
            Self::Again(failure) | Self::Final(failure) => failure,
        }
    }
}

/// Whether `code` may enter a path.
#[must_use]
pub fn is_code(code: &str) -> bool {
    (1..=MAX_CODE).contains(&code.len())
        && code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

/// An unreachable source or a 5xx is tried once more after `retry_after`.
///
/// # Errors
/// [`ConvertFailure`]; a code that is not plain is `Refused` before any request.
pub async fn convert(
    transport: &Transport,
    declaration: &Declaration,
    code: &str,
    (now, retry_after): (DateTime<Utc>, Duration),
) -> Result<Converted, ConvertFailure> {
    let registration = registration(declaration).ok_or(ConvertFailure::NotRegistrable)?;
    if !is_code(code) {
        return Err(ConvertFailure::Refused);
    }
    let target = Request::get(&(registration.conversion_path)(code), &[])
        .map_err(|_| ConvertFailure::Unusable)?;
    let url = transport
        .url(declaration.api_base, &target)
        .map_err(|_| ConvertFailure::Unusable)?;
    let first = once(transport, &registration, url.clone(), now).await;
    let outcome = match first {
        Err(Miss::Again(_)) => {
            tokio::time::sleep(retry_after).await;
            once(transport, &registration, url, now).await
        }
        done => done,
    };
    outcome.map_err(Miss::failure)
}

async fn once(
    transport: &Transport,
    registration: &Registration,
    url: reqwest::Url,
    now: DateTime<Utc>,
) -> Result<Converted, Miss> {
    let answer = transport.convert(url).await.map_err(|fault| match fault {
        Fault::Network => Miss::Again(ConvertFailure::Unreachable),
        Fault::Refused => Miss::Final(ConvertFailure::Unusable),
    })?;
    match (judge(answer.status, answer.limits, now), answer.status) {
        (Verdict::RateLimited(until), _) => Err(Miss::Final(ConvertFailure::RateLimited(until))),
        (Verdict::Refused, _) | (Verdict::Usable, 400..=499) => {
            Err(Miss::Final(ConvertFailure::Refused))
        }
        (Verdict::Usable, 500..=599) => Err(Miss::Again(ConvertFailure::Unreachable)),
        (Verdict::Usable, 201) => match &answer.content {
            Content::Body(body) => (registration.parse_converted)(body)
                .map_err(|_| Miss::Final(ConvertFailure::Unusable)),
            Content::TooLarge => Err(Miss::Final(ConvertFailure::Unusable)),
        },
        (Verdict::Usable, _) => Err(Miss::Final(ConvertFailure::Unusable)),
    }
}

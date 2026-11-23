//! How the hub gets a token for a source and sends requests with it
//! (SRC-002, PRJ-018, K-E1 Sections 7 and 7b).

pub mod account_key;
pub mod app_key;
pub mod conversion;
pub mod installations;
pub mod stored_key;
pub mod tokens;
pub mod transport;
pub mod verdict;

use chrono::{DateTime, Utc};
use ring::rand::SecureRandom;
use source_exchange::{Acquisition, Declaration, Minted, Minting, Registration, Request};

use self::{
    app_key::AppKey,
    transport::{Content, Fault, Transport},
    verdict::{Verdict, judge},
};

/// Why no token was minted. Fixed words (LOG-007).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MintFailure {
    #[error("the source refused the key")]
    Refused,
    #[error("the source limits requests")]
    RateLimited(DateTime<Utc>),
    #[error("the source could not be reached")]
    Unreachable,
    #[error("the source's reply to the token request could not be used")]
    Unusable,
    #[error("the source does not take a token pull with an App key")]
    NotAppKey,
    #[error("the App token could not be signed")]
    Unsigned,
}

pub(super) fn registration(declaration: &Declaration) -> Option<Registration> {
    // Acquisition and Minting are non_exhaustive: a later variant has no App to make.
    let Acquisition::TokenPull(Minting::AppKey(app)) = declaration.acquisition else {
        return None;
    };
    app.registration
}

/// # Errors
/// [`MintFailure`].
pub async fn mint(
    transport: &Transport,
    declaration: &Declaration,
    key: &AppKey,
    (now, random): (DateTime<Utc>, &(dyn SecureRandom + Sync)),
) -> Result<Minted, MintFailure> {
    let Acquisition::TokenPull(Minting::AppKey(app)) = declaration.acquisition else {
        return Err(MintFailure::NotAppKey);
    };
    // The mint is a POST; the path goes through the same check as a provider's GET.
    let target = Request::get(&(app.mint_path)(key.installation_id()), &[])
        .map_err(|_| MintFailure::Unusable)?;
    let url = transport
        .url(declaration.api_base, &target)
        .map_err(|_| MintFailure::Unusable)?;
    let app_token = key.token(now, random).map_err(|_| MintFailure::Unsigned)?;
    let answer = transport
        .mint(url, &app_token)
        .await
        .map_err(|fault| match fault {
            Fault::Network => MintFailure::Unreachable,
            Fault::Refused => MintFailure::Unusable,
        })?;
    match (judge(answer.status, answer.limits, now), answer.status) {
        (Verdict::RateLimited(until), _) => Err(MintFailure::RateLimited(until)),
        (Verdict::Refused, _) | (Verdict::Usable, 400..=499) => Err(MintFailure::Refused),
        (Verdict::Usable, 200..=299) => match &answer.content {
            Content::Body(body) => (app.parse_minted)(body).map_err(|_| MintFailure::Unusable),
            Content::TooLarge => Err(MintFailure::Unusable),
        },
        (Verdict::Usable, _) => Err(MintFailure::Unusable),
    }
}

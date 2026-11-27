//! GitHub's installation token request for one repository checkout.
use std::num::NonZeroU64;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy)]
pub struct CheckoutScope(NonZeroU64);

impl CheckoutScope {
    pub const REVOCATION_PATH: &str = "/installation/token";

    #[must_use]
    pub const fn new(repository_id: NonZeroU64) -> Self {
        Self(repository_id)
    }

    #[must_use]
    pub fn request(self) -> impl Serialize {
        Request {
            repository_ids: [self.0],
            permissions: RequestedPermissions { contents: "read" },
        }
    }

    /// The transport may revoke a returned token even if GitHub gave it the wrong scope.
    /// A borrowed token stays in the transport's zeroized response body.
    #[must_use]
    pub fn read(self, body: &[u8]) -> Option<CheckoutTokenReply<'_>> {
        let token = serde_json::from_slice::<Token<'_>>(body).ok()?.token;
        if token.is_empty()
            || token.len() > 16 * 1024
            || !token.bytes().all(|b| b.is_ascii_graphic())
        {
            return None;
        }
        let scope = serde_json::from_slice::<Scope<'_>>(body).ok();
        let expires_at = scope
            .filter(|reply| {
                reply.repository_selection == "selected"
                    && reply.repositories[0].id == self.0
                    && reply.permissions.contents == "read"
                    && reply
                        .permissions
                        .metadata
                        .is_none_or(|level| level == "read")
            })
            .map(|reply| reply.expires_at);
        Some(CheckoutTokenReply { token, expires_at })
    }
}

pub struct CheckoutTokenReply<'a> {
    pub token: &'a str,
    /// Present only if GitHub confirmed precisely the requested authorization.
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct Request {
    repository_ids: [NonZeroU64; 1],
    permissions: RequestedPermissions,
}

#[derive(Serialize)]
struct RequestedPermissions {
    contents: &'static str,
}

#[derive(Deserialize)]
struct Token<'a> {
    token: &'a str,
}

#[derive(Deserialize)]
struct Scope<'a> {
    repository_selection: &'a str,
    repositories: [Repository; 1],
    #[serde(borrow)]
    permissions: Permissions<'a>,
    expires_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct Repository {
    id: NonZeroU64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Permissions<'a> {
    contents: &'a str,
    metadata: Option<&'a str>,
}

use std::{fmt, sync::Arc};

use chrono::{DateTime, TimeDelta, Utc};
use source_exchange::{Body, Response};

/// Whether a body says it will not change soon; pure.
pub type Settles = fn(body: &[u8]) -> bool;

/// Whether a body may be kept at all; pure.
pub type Holdable = fn(body: &[u8]) -> bool;

#[derive(Clone, Copy, Debug)]
pub struct Keep {
    fresh_for: Option<TimeDelta>,
    settled: Option<(TimeDelta, Settles)>,
    holds: Option<Holdable>,
    forced: bool,
}

impl Keep {
    #[must_use]
    pub const fn for_seconds(seconds: i64) -> Self {
        Self {
            fresh_for: Some(TimeDelta::seconds(seconds)),
            settled: None,
            holds: None,
            forced: false,
        }
    }

    /// Never kept or served from the cache, fresh or stale: file content (K-E1 Section 13.1).
    #[must_use]
    pub const fn never() -> Self {
        Self {
            fresh_for: None,
            settled: None,
            holds: None,
            forced: false,
        }
    }

    #[must_use]
    pub const fn only_when(self, holds: Holdable) -> Self {
        Self {
            holds: Some(holds),
            ..self
        }
    }

    #[must_use]
    pub const fn longer_when(self, seconds: i64, settled: Settles) -> Self {
        Self {
            settled: Some((TimeDelta::seconds(seconds), settled)),
            ..self
        }
    }

    /// `true`: read the source again unless the held answer is under five seconds old.
    #[must_use]
    pub const fn fresh(self, fresh: bool) -> Self {
        Self {
            forced: fresh,
            ..self
        }
    }

    pub(super) const fn is_kept(self) -> bool {
        self.fresh_for.is_some()
    }

    pub(super) const fn is_forced(self) -> bool {
        self.forced
    }

    pub(super) fn fresh_for(self, body: &[u8]) -> Option<TimeDelta> {
        let short = self.fresh_for?;
        if self.holds.is_some_and(|holds| !holds(body)) {
            return None;
        }
        Some(match self.settled {
            Some((long, settled)) if settled(body) => long,
            _ => short,
        })
    }
}

/// Whether a change may spend the sync's reserved requests (K-E1 Section 13.1).
/// The caller decides; the engine knows no token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reserve {
    /// The operator's own change (a login or browser token): it may go down to 0.
    Spendable,
    /// A script's or an agent's change (an API key): it stops at the reserve.
    Kept,
}

/// Fixed words only: never a token, a path, a body or a reply of the source (LOG-007).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LiveFailure {
    #[error("no source account has this ID")]
    NotFound,
    #[error("the account is paused")]
    Paused,
    #[error("the source refused the key")]
    KeyRefused,
    #[error("the source limits requests")]
    Limited { until: DateTime<Utc> },
    /// Nothing was sent: the source could not be reached.
    #[error("the source could not be reached")]
    Unreachable,
    #[error("the source's answer could not be used")]
    Unusable,
    /// Nothing was sent: the path, the host or the size of the body is not one the hub sends.
    #[error("the hub does not send this request")]
    Refused,
    #[error("the source makes no changes")]
    NoChanges,
    #[error("the hub is still busy with this account's requests")]
    Busy,
    #[error("the hub could not read the account")]
    Store,
}

impl LiveFailure {
    /// Whether a held answer may stand in for the failed read.
    pub(super) const fn passes(self) -> bool {
        matches!(
            self,
            Self::Limited { .. } | Self::Unreachable | Self::Unusable | Self::Busy | Self::Store
        )
    }
}

#[derive(Clone)]
pub struct Reply {
    pub(super) status: u16,
    /// `None`: larger than the hub reads.
    pub(super) body: Option<Arc<[u8]>>,
    pub(super) at: DateTime<Utc>,
    pub(super) problem: Option<LiveFailure>,
}

impl Reply {
    #[must_use]
    pub fn response(&self) -> Response<'_> {
        Response {
            link: None,
            status: self.status,
            body: self.body.as_deref().map_or(Body::TooLarge, Body::Read),
        }
    }

    #[must_use]
    pub const fn at(&self) -> DateTime<Utc> {
        self.at
    }

    /// The newest read failed and this is the last good answer, at most an hour old.
    #[must_use]
    pub const fn stale(&self) -> bool {
        self.problem.is_some()
    }

    #[must_use]
    pub const fn problem(&self) -> Option<LiveFailure> {
        self.problem
    }
}

/// The body is never shown (LOG-007): only its length.
impl fmt::Debug for Reply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reply")
            .field("status", &self.status)
            .field("bytes", &self.body.as_ref().map(|body| body.len()))
            .field("at", &self.at)
            .field("problem", &self.problem)
            .finish()
    }
}

/// Whether an account can be read now (design B.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    Ready,
    NeedsReconnect,
    Paused,
    NotFound,
}

//! Session token sums, cost estimates and recency (AIS-021, AIS-025, AIS-026).

use chrono::{DateTime, TimeDelta, Utc};

use super::{
    model::{Session, StoredSession, TokenSums},
    prices,
};

/// Two count periods of the daemon, which counts every 15 minutes (AIS-026).
pub const RECENT: TimeDelta = TimeDelta::minutes(30);

/// The estimate is `None` when no token has a price (AIS-021).
#[must_use]
pub fn priced(stored: StoredSession) -> Session {
    let mut tokens = TokenSums::default();
    let mut estimated_cost_usd = None;
    let mut unpriced_tokens = 0.0;
    for (model, sums) in &stored.models {
        tokens = tokens.plus(*sums);
        if sums.all() > 0.0 {
            match prices::estimate(model, *sums) {
                Some(cost) => estimated_cost_usd = Some(estimated_cost_usd.unwrap_or(0.0) + cost),
                None => unpriced_tokens += sums.all(),
            }
        }
    }
    Session {
        stored,
        tokens,
        estimated_cost_usd,
        unpriced_tokens,
    }
}

#[must_use]
pub fn is_recent(last_use: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    now - last_use <= RECENT
}

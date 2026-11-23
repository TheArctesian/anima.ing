//! The status rule of a domain expiry check (D-29).

use chrono::{DateTime, Utc};

use crate::fleet::checks::{
    model::whole_days,
    probe::outcome::{Failure, Judgement, ProbeResult, clean_message},
};

/// No response time: the RDAP server's speed says nothing about the domain.
pub(super) fn judge(
    not_after: Option<DateTime<Utc>>,
    warn_days: u32,
    now: DateTime<Utc>,
) -> ProbeResult {
    let Some(not_after) = not_after else {
        return ProbeResult::unjudged(&Failure::NoExpiryDate);
    };
    let days = whole_days(not_after, now);
    let mut result = if not_after <= now {
        ProbeResult::failed(&Failure::DomainExpired)
    } else if i64::from(days) < i64::from(warn_days) {
        ProbeResult {
            message: Some(clean_message(&Failure::DomainExpiring.to_string())),
            judgement: Judgement::Expiring,
            ..ProbeResult::up_unmeasured()
        }
    } else {
        ProbeResult::up_unmeasured()
    };
    result.domain_not_after = Some(not_after);
    result
}

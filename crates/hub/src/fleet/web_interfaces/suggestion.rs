//! Which HTTP checks are offered as interfaces of a device (WEB-002, D-7). Pure.

use std::collections::HashSet;

use reqwest::Url;

use super::{
    model::{Suggestion, WebInterface},
    rules,
};
use crate::ids::CheckId;

/// Addresses compare as parsed URLs; one the URL rule refuses is left out, as a user name in it would show (LOG-007).
pub(super) fn filter(
    http_checks: &[(CheckId, String)],
    interfaces: &[WebInterface],
    dismissed: &HashSet<CheckId>,
) -> Vec<Suggestion> {
    let taken: Vec<Url> = interfaces
        .iter()
        .filter_map(|interface| Url::parse(&interface.url).ok())
        .collect();
    http_checks
        .iter()
        .filter(|(check, _)| !dismissed.contains(check))
        .filter_map(|(check, text)| {
            let (url, parsed) = rules::parse_url(text).ok()?;
            let label = rules::host_label(parsed.host_str()?);
            (!taken.contains(&parsed)).then_some(Suggestion {
                check: *check,
                url,
                label,
            })
        })
        .collect()
}

pub(super) fn same_url(a: &str, b: &str) -> bool {
    matches!((Url::parse(a), Url::parse(b)), (Ok(a), Ok(b)) if a == b)
}

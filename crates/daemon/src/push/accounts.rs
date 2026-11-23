//! The accounts a batch carries (K8 Section 4, AIS-022, THR-149); the vendor's identifier is read only to make the key.

use base64::Engine as _;
use chrono::DateTime;
use daemon_core::agent_usage::account::{
    Account, Address, Limit, MAX_ACCOUNTS, MAX_LIMITS, SALT_BYTES, account_key, is_plan, is_window,
};
use daemon_core::wire::BatchAccount;

use crate::sampling::agent_usage::ProgramAccount;

/// The hub's `accountSalt`; `None` for text that is not standard base64 of [`SALT_BYTES`] bytes.
#[must_use]
pub fn salt(text: &str) -> Option<[u8; SALT_BYTES]> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text)
        .ok()?;
    bytes.try_into().ok()
}

/// Empty with no salt yet, which asks the hub for its salt (K8 Section 8.2).
#[must_use]
pub fn outgoing(salt: Option<&[u8; SALT_BYTES]>, accounts: &[ProgramAccount]) -> Vec<BatchAccount> {
    let Some(salt) = salt else {
        return Vec::new();
    };
    let mut sent: Vec<BatchAccount> = Vec::new();
    for one in accounts {
        let facts = &one.facts;
        let Some(id) = facts.id.as_ref().filter(|_| !facts.signed_out) else {
            continue;
        };
        if sent.len() == MAX_ACCOUNTS || sent.iter().any(|done| done.source == one.source.as_str())
        {
            continue;
        }
        let account = Account {
            source: one.source,
            key: account_key(salt, one.source, id.reveal()),
            address: facts
                .name
                .as_ref()
                .and_then(|name| Address::parse(name.reveal())),
            plan: facts.plan.clone().filter(|plan| is_plan(plan)),
            limits: facts
                .limits
                .iter()
                .filter(|limit| is_window(&limit.window))
                .take(MAX_LIMITS)
                .map(|limit| Limit {
                    window: limit.window.clone(),
                    used_percent: limit.used_percent.map(|used| used.min(100)),
                    resets_at: limit
                        .resets_at
                        .and_then(|at| DateTime::from_timestamp(at, 0)),
                    reached: limit.reached,
                })
                .collect(),
        };
        sent.push(BatchAccount::from(&account));
    }
    sent
}

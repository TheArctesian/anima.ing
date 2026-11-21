//! The accounts of the AI agent programs (THR-149, AIS-022; K8 Section 4
//! `agentUsage.accounts`, Section 8.2 `accountSalt`): the newest report of each device and
//! source, and the hub's account salt. The address of an account is personal data: it is in no
//! log line (its `Debug` shows no text) and goes with the row of its device.
//!
//! [`record`] stores a report in the transaction of its batch. [`salt`] gives the salt, made
//! once. [`list`] reads the reports, and [`grouped`] puts the devices of one key under one
//! account, with the plan and the limit windows of the newest report: the limit is the
//! account's, so each device of it reports the same one.

pub mod graphql;
mod store;

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use daemon_core::agent_usage::Source;
use daemon_core::agent_usage::account::{Account, Address, Limit};
use sqlx::{PgConnection, PgPool};

use super::Error;
use crate::ids::DeviceId;

/// One report of one device: the account it uses for `source`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reported {
    pub device: DeviceId,
    pub account: Account,
    pub reported_at: DateTime<Utc>,
}

/// One account and the devices that use it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountGroup {
    pub key: String,
    pub source: Source,
    /// The email address of the newest report.
    pub address: Option<Address>,
    pub plan: Option<String>,
    pub limits: Vec<Limit>,
    /// The time of the newest report, which gave the address, the plan and the limits.
    pub reported_at: DateTime<Utc>,
    /// The devices that use the account, in the order of their newest report first.
    pub devices: Vec<DeviceId>,
}

/// Stores `accounts`, reported by `device` at `now`, in the caller's transaction: each one
/// replaces the earlier report of the device and its source, limits included, and the report of
/// each source that the list does not name is deleted (a program signed out, or the device
/// turned the accounts off; THR-149).
///
/// # Errors
/// `Database` when a statement fails; nothing of the batch is then kept.
pub async fn record(
    conn: &mut PgConnection,
    device: DeviceId,
    accounts: &[Account],
    now: DateTime<Utc>,
) -> Result<(), Error> {
    for account in accounts {
        store::write(conn, device, account, now).await?;
    }
    let named: Vec<&str> = accounts.iter().map(|one| one.source.as_str()).collect();
    store::forget_others(conn, device, &named).await
}

/// The account salt of this hub: 32 random bytes, made by the first call and the same ever after.
///
/// # Errors
/// `Database` when a statement fails.
pub async fn salt(db: &PgPool) -> Result<[u8; 32], Error> {
    let made: [u8; 32] = rand::random();
    store::salt(db, &made).await
}

/// Every stored report, of every device.
///
/// # Errors
/// `Database` when a query fails; `Unreadable` for a stored source this version does not know.
pub async fn list(db: &PgPool) -> Result<Vec<Reported>, Error> {
    store::read(db).await
}

/// The reports put under their keys: one group for each key, with the address, the plan and the
/// limits of its newest report, its devices newest first; the groups newest first.
#[must_use]
pub fn grouped(mut reports: Vec<Reported>) -> Vec<AccountGroup> {
    reports.sort_by_key(|one| std::cmp::Reverse(one.reported_at));
    let mut groups: BTreeMap<String, AccountGroup> = BTreeMap::new();
    for report in reports {
        let group = groups
            .entry(report.account.key.clone())
            .or_insert_with(|| AccountGroup {
                key: report.account.key.clone(),
                source: report.account.source,
                address: report.account.address.clone(),
                plan: report.account.plan.clone(),
                limits: report.account.limits.clone(),
                reported_at: report.reported_at,
                devices: Vec::new(),
            });
        if !group.devices.contains(&report.device) {
            group.devices.push(report.device);
        }
    }
    let mut groups: Vec<AccountGroup> = groups.into_values().collect();
    groups.sort_by(|one, other| {
        other
            .reported_at
            .cmp(&one.reported_at)
            .then(one.key.cmp(&other.key))
    });
    groups
}

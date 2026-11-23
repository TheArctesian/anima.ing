//! The domain expiry probe (spec 6.4, CHK-009, D-29); every connection leaves through [`egress`] (P3-003, P-6).
//! An RDAP that cannot be reached or read is Unknown, never Down; Down comes only from the registry.
//! No message holds a URL, an address or any of the reply (LOG-007).

mod bootstrap;
mod cache;
mod egress;
mod expiry;
mod reply;
#[cfg(test)]
pub(in crate::fleet::checks) mod tests;

use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use reqwest::Url;

use super::outcome::{Failure, ProbeResult};
use crate::fleet::targets::{AddressPolicy, TargetError};
use bootstrap::Bootstrap;
use cache::Cache;
use egress::{Egress, FetchError};

const BOOTSTRAP_URL: &str = "https://data.iana.org/rdap/dns.json";
/// The file is about 40 KiB.
const BOOTSTRAP_CAP: usize = 1024 * 1024;
const REPLY_CAP: usize = 256 * 1024;

#[derive(Clone, Copy, Debug)]
pub(in crate::fleet::checks) struct Lookup<'a> {
    pub(in crate::fleet::checks) domain: &'a str,
    pub(in crate::fleet::checks) warn_days: u32,
    pub(in crate::fleet::checks) timeout: Duration,
}

/// One lives in the scheduler, so the cache outlives each attempt; it is never global.
#[derive(Debug)]
pub(in crate::fleet::checks) struct Rdap {
    egress: Egress,
    bootstrap_url: Url,
    cache: Cache,
}

impl Rdap {
    pub(in crate::fleet::checks) fn new(policy: AddressPolicy) -> Self {
        Self {
            egress: Egress::new(policy),
            bootstrap_url: Url::parse(BOOTSTRAP_URL).expect("the bootstrap address is a URL"),
            cache: Cache::default(),
        }
    }

    #[cfg(test)]
    pub(in crate::fleet::checks) fn for_tests(
        policy: AddressPolicy,
        root: Vec<u8>,
        bootstrap_url: Url,
    ) -> Self {
        Self {
            egress: Egress::new(policy).trusting(root),
            bootstrap_url,
            cache: Cache::default(),
        }
    }

    pub(in crate::fleet::checks) async fn observe(
        &self,
        lookup: Lookup<'_>,
        now: DateTime<Utc>,
    ) -> ProbeResult {
        let domain = lookup.domain.to_ascii_lowercase();
        match tokio::time::timeout(lookup.timeout, self.expiry(&domain, now)).await {
            Ok(Ok(not_after)) => expiry::judge(not_after, lookup.warn_days, now),
            Ok(Err(Failure::DomainNotFound)) => ProbeResult::failed(&Failure::DomainNotFound),
            Ok(Err(failure)) => ProbeResult::unjudged(&failure),
            Err(_) => ProbeResult::unjudged(&Failure::TimedOut),
        }
    }

    async fn expiry(
        &self,
        domain: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>, Failure> {
        let bootstrap = self.bootstrap(now).await?;
        let query = bootstrap.query(domain)?;
        let body = match self.egress.get(&query, REPLY_CAP).await {
            Err(FetchError::Status(404)) => return Err(Failure::DomainNotFound),
            fetched => fetched.map_err(failure)?,
        };
        reply::expiration(&body).map_err(|_| Failure::RdapUnreadable)
    }

    async fn bootstrap(&self, now: DateTime<Utc>) -> Result<Arc<Bootstrap>, Failure> {
        if let Some(kept) = self.cache.fresh(now) {
            return Ok(kept);
        }
        let body = self
            .egress
            .get(&self.bootstrap_url, BOOTSTRAP_CAP)
            .await
            .map_err(failure)?;
        let read = Arc::new(Bootstrap::parse(&body).map_err(|_| Failure::RdapUnreadable)?);
        self.cache.keep(now, Arc::clone(&read));
        Ok(read)
    }
}

fn failure(error: FetchError) -> Failure {
    match error {
        FetchError::Target(TargetError::Refused) => Failure::NotPermitted,
        FetchError::Target(TargetError::Unresolved) => Failure::Unresolved,
        FetchError::Address => Failure::RdapNotHttps,
        FetchError::Redirects => Failure::TooManyRedirects,
        FetchError::Status(code) => Failure::Status(code),
        FetchError::TooLong => Failure::RdapTooLong,
        FetchError::Tls => Failure::Handshake,
        FetchError::Setup | FetchError::Connect | FetchError::Exchange => Failure::Request,
    }
}

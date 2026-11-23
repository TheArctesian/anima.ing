//! The IANA RDAP bootstrap file for domain names (RFC 9224).
//! It comes from the network, so only well-formed entries and HTTPS base URLs with a host and no user name are kept.

use reqwest::Url;
use serde::Deserialize;
use serde_json::Value;

use crate::fleet::checks::probe::outcome::Failure;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("the RDAP bootstrap file cannot be read")]
pub(super) struct Unreadable;

#[derive(Debug, PartialEq)]
struct Registry {
    suffixes: Vec<String>,
    /// Ends with `/`, so `domain/<name>` joins under it.
    base: Option<Url>,
}

#[derive(Debug, PartialEq)]
pub(in crate::fleet::checks::probe::rdap) struct Bootstrap {
    services: Vec<Registry>,
}

#[derive(Deserialize)]
struct File {
    services: Vec<Value>,
}

impl Bootstrap {
    pub(super) fn parse(body: &[u8]) -> Result<Self, Unreadable> {
        let file: File = serde_json::from_slice(body).map_err(|_| Unreadable)?;
        Ok(Self {
            services: file.services.iter().filter_map(registry).collect(),
        })
    }

    /// The suffix that matches the most labels decides; a shorter suffix never stands in for it.
    pub(super) fn query(&self, domain: &str) -> Result<Url, Failure> {
        let best = self
            .services
            .iter()
            .flat_map(|service| service.suffixes.iter().map(move |suffix| (suffix, service)))
            .filter(|(suffix, _)| covers(suffix, domain))
            .max_by_key(|(suffix, _)| suffix.split('.').count())
            .map(|(_, service)| service)
            .ok_or(Failure::NoRdapService)?;
        let base = best.base.as_ref().ok_or(Failure::RdapNotHttps)?;
        base.join(&format!("domain/{domain}"))
            .map_err(|_| Failure::NoRdapService)
    }
}

fn covers(suffix: &str, domain: &str) -> bool {
    domain == suffix
        || domain
            .strip_suffix(suffix)
            .is_some_and(|head| head.ends_with('.'))
}

fn registry(entry: &Value) -> Option<Registry> {
    let [suffixes, urls, ..] = entry.as_array()?.as_slice() else {
        return None;
    };
    let suffixes: Vec<String> = strings(suffixes)?
        .map(|suffix| suffix.trim_matches('.').to_ascii_lowercase())
        .filter(|suffix| !suffix.is_empty())
        .collect();
    let base = strings(urls)?.find_map(https_base);
    (!suffixes.is_empty()).then_some(Registry { suffixes, base })
}

fn strings(value: &Value) -> Option<impl Iterator<Item = &str>> {
    let list = value.as_array()?;
    list.iter()
        .all(Value::is_string)
        .then(|| list.iter().filter_map(Value::as_str))
}

fn https_base(text: &str) -> Option<Url> {
    let mut url = Url::parse(text).ok()?;
    let plain = url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none();
    if !plain {
        return None;
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Some(url)
}

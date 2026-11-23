//! The caller connects to a returned address and never resolves the name again, so DNS rebinding cannot change the target.

use std::net::IpAddr;

use super::{ProbeEnv, outcome::Failure};
use crate::fleet::{
    checks::config::IpVersion,
    targets::{self, TargetError},
};

pub(super) async fn allowed_addresses(
    host: &str,
    family: Option<IpVersion>,
    env: &ProbeEnv,
) -> Result<Vec<IpAddr>, Failure> {
    let mut found = lookup(host, env).await?;
    if let Some(version) = family {
        found.retain(|ip| {
            matches!(
                (version, ip),
                (IpVersion::V4, IpAddr::V4(_)) | (IpVersion::V6, IpAddr::V6(_))
            )
        });
        if found.is_empty() {
            return Err(Failure::NoAddress);
        }
    }
    found.retain(|ip| env.policy.permits(*ip));
    if found.is_empty() {
        Err(Failure::NotPermitted)
    } else {
        Ok(found)
    }
}

pub(super) fn off_link(ip: IpAddr) -> Result<IpAddr, Failure> {
    if targets::on_link(ip) {
        Err(Failure::TcpOnly)
    } else {
        Ok(ip)
    }
}

/// An IP literal in any number of brackets is read as the check editor accepts it.
async fn lookup(host: &str, env: &ProbeEnv) -> Result<Vec<IpAddr>, Failure> {
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    let host = if bare.parse::<IpAddr>().is_ok() {
        bare
    } else {
        host
    };
    targets::lookup(host, &env.overrides.hosts)
        .await
        .map_err(|error| match error {
            TargetError::Unresolved | TargetError::Refused => Failure::Unresolved,
        })
}

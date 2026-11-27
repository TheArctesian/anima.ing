//! Local endpoint leases. The remote adapter must verify the caller's grant first.
mod store;
use super::{Error, Result, workspace};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub use agent_threads::managed::preview::Binding;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lease {
    schema: u32,
    pub binding: Binding,
    pub revision: u64,
    revoked: bool,
}

pub async fn open(binding: Binding) -> Result<Value> {
    let now = now()?;
    if [
        binding.endpoint,
        binding.principal,
        binding.grant,
        binding.workspace,
    ]
    .iter()
    .any(Uuid::is_nil)
        || binding.generation == 0
        || binding.port == 0
        || binding.expires_at <= now
        || binding.expires_at > now.saturating_add(600)
    {
        return Err(Error::Refused(
            "Preview identity or its ten-minute lease is invalid.",
        ));
    }
    let _lock = store::lock(binding.endpoint)?;
    workspace::check_preview(binding.workspace, binding.generation, binding.principal)?;
    if let Some(existing) = store::read(binding.endpoint)? {
        if existing.binding != binding {
            return Err(Error::Refused(
                "The preview identity belongs to another request.",
            ));
        }
        return Ok(serde_json::to_value(existing)?);
    }
    workspace::connection(binding.workspace, binding.generation).await?;
    let lease = Lease {
        schema: 1,
        binding,
        revision: 1,
        revoked: false,
    };
    store::save(&lease)?;
    Ok(serde_json::to_value(lease)?)
}

pub fn revoke(endpoint: Uuid, revision: u64) -> Result<Value> {
    let _lock = store::lock(endpoint)?;
    let mut lease = required(endpoint)?;
    if lease.revision != revision {
        return Err(Error::Refused(
            "Refresh the preview revision before closing it.",
        ));
    }
    lease.revoked = true;
    store::save(&lease)?;
    Ok(serde_json::to_value(lease)?)
}

pub fn close_for(endpoint: Uuid, revision: u64, principal: Uuid) -> Result<Value> {
    let lease = required(endpoint)?;
    if lease.binding.principal != principal {
        return Err(Error::Refused("The preview belongs to another principal."));
    }
    revoke(endpoint, revision)
}

pub fn resolve(endpoint: Uuid, revision: u64, principal: Uuid) -> Result<Lease> {
    let lease = required(endpoint)?;
    if lease.revision != revision || lease.binding.principal != principal {
        return Err(Error::Refused(
            "The preview does not belong to this caller and revision.",
        ));
    }
    live(&lease)?;
    Ok(lease)
}

pub fn live(expected: &Lease) -> Result<()> {
    let current = required(expected.binding.endpoint)?;
    if current != *expected || current.revoked || current.binding.expires_at <= now()? {
        return Err(Error::Refused("The preview has expired or was revoked."));
    }
    let binding = current.binding;
    workspace::check_preview(binding.workspace, binding.generation, binding.principal)
}

pub async fn until_closed(lease: &Lease) -> Result<()> {
    let remaining = lease.binding.expires_at.saturating_sub(now()?).min(600);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(remaining);
    loop {
        live(lease)?;
        if tokio::time::Instant::now() >= deadline {
            return Err(Error::Refused("The preview lease expired."));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn required(endpoint: Uuid) -> Result<Lease> {
    store::read(endpoint)?.ok_or(Error::Refused("The preview endpoint is unavailable."))
}
fn now() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .map_err(|_| Error::Refused("The node clock cannot validate the preview lease."))
}

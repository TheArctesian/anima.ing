//! Admission arithmetic. Observation is supplied explicitly; UI samples are not capacity.
use crate::Error;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    pub memory_bytes: u64,
    pub cpu_millis: u64,
    pub tasks: u64,
}

impl Resources {
    pub(crate) fn add(self, other: Self) -> Result<Self, Error> {
        let overflow = || Error::Refused("Resource accounting exceeded its limit.");
        Ok(Self {
            memory_bytes: self
                .memory_bytes
                .checked_add(other.memory_bytes)
                .ok_or_else(overflow)?,
            cpu_millis: self
                .cpu_millis
                .checked_add(other.cpu_millis)
                .ok_or_else(overflow)?,
            tasks: self.tasks.checked_add(other.tasks).ok_or_else(overflow)?,
        })
    }
}

#[derive(Clone, Copy)]
pub struct Capacity {
    pub total_memory: u64,
    pub available_memory: u64,
    pub cpu_millis: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Policy {
    pub schema: u32,
    pub protected_memory_percent: u64,
    pub maximum_agent_memory_percent: u64,
    pub maximum_agent_cpu_percent: u64,
    pub maximum_agent_tasks: u64,
    pub agent_slice: String,
    pub hosting_slice: String,
}

/// Remaining hosting-safe resources for a new reservation, not a capacity grant.
/// # Errors
/// Invalid packaged admission policy.
pub fn available(policy: &Policy, capacity: Capacity, used: Resources) -> Result<Resources, Error> {
    if policy.schema != 1
        || policy.agent_slice != "anima-agents.slice"
        || policy.hosting_slice != "anima-hosting.slice"
        || policy.protected_memory_percent < 50
        || policy.protected_memory_percent >= 100
        || policy.maximum_agent_memory_percent == 0
        || policy.maximum_agent_memory_percent > 100 - policy.protected_memory_percent
        || policy.maximum_agent_cpu_percent == 0
        || policy.maximum_agent_cpu_percent > 50
        || policy.maximum_agent_tasks == 0
        || policy.maximum_agent_tasks > 1024
    {
        return Err(Error::Refused("The node admission policy is invalid."));
    }
    let memory_limit = capacity.total_memory / 100 * policy.maximum_agent_memory_percent;
    let protected = capacity.total_memory / 100 * policy.protected_memory_percent;
    let cpu_limit = capacity.cpu_millis / 100 * policy.maximum_agent_cpu_percent;
    Ok(Resources {
        memory_bytes: memory_limit
            .saturating_sub(used.memory_bytes)
            .min(capacity.available_memory.saturating_sub(protected)),
        cpu_millis: cpu_limit.saturating_sub(used.cpu_millis),
        tasks: policy.maximum_agent_tasks.saturating_sub(used.tasks),
    })
}

/// Only resource eligibility; the owning work route must authorize the principal,
/// workspace generation and provider before asking for a reservation.
/// # Errors
/// Invalid policy, unavailable capacity or a request that would consume protected capacity.
pub fn admit(
    policy: &Policy,
    capacity: Capacity,
    used: Resources,
    request: Resources,
) -> Result<(), Error> {
    let free = available(policy, capacity, used)?;
    if request.memory_bytes < 64 * 1024 * 1024 || request.cpu_millis == 0 || request.tasks == 0 {
        return Err(Error::Refused(
            "The work request must name nonzero resource limits.",
        ));
    }
    used.add(request)?;
    if request.memory_bytes > free.memory_bytes
        || request.cpu_millis > free.cpu_millis
        || request.tasks > free.tasks
    {
        return Err(Error::Refused(
            "Hosting reserves this capacity. Queue the work or choose another node.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;

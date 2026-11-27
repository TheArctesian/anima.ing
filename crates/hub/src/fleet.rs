//! The device-manager domains (REPO-E-001, OD-E5); K3 lists what one domain may use from another.

pub mod access;
pub mod agent_threads;
pub mod agent_usage;
pub mod builds;
pub mod checks;
pub(crate) mod code_previews;
pub mod compute;
pub mod credentials;
pub mod devices;
pub mod events;
pub mod link;
pub mod maintenance;
pub mod metrics;
pub mod notifications;
pub mod os_updates;
pub mod retention;
pub mod rollout;
pub mod sessions;
pub mod ssh;
pub mod targets;
pub mod trust;
pub mod vpn;
pub mod web_interfaces;

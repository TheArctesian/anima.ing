//! The Anima push daemon: it pushes this computer's metrics to the hub (MET-011 to MET-020,
//! `contracts/daemon-push.md`).

pub mod cli;
pub mod config;
pub mod hub;
pub mod keychain_wait;
pub mod keys;
#[cfg(unix)]
pub mod model;
pub mod push;
pub mod readers;
pub mod report;
pub mod runtime;
pub mod sampling;
#[cfg(unix)]
pub mod threads;
pub mod token_store;
#[cfg_attr(windows, path = "units/windows.rs")]
pub mod units;

#[cfg(test)]
mod test_links;
pub mod work;

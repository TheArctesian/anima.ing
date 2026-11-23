//! Connect GitHub with an App the hub makes from a manifest (PRJ-039 to PRJ-042, K-E1 Section 7a).
//! One attempt exists at a time, in hub memory; the hub keeps only hashes of the ticket, cookie and
//! state, and the private key goes only to `establish`.

pub mod attempt;
pub mod attempts;
pub mod binding;
mod exchange;
mod finish;
pub mod installation;
mod pages;
mod query;
pub mod routes;
#[cfg(test)]
mod tests;
pub mod trail;
pub mod words;

use source_exchange::Registration;

use super::{acquisition, intake::Registry};
use crate::config::Config;

pub const GITHUB: &str = "github";

/// `None` when no browser can open this hub: link mode with no public origin (HMD-070).
#[must_use]
pub fn browser_origin(config: &Config) -> Option<&str> {
    let unaddressed = config.link_socket.is_some() && !config.public_origin_given;
    (!unaddressed).then_some(config.public_origin.as_str())
}

/// How GitHub makes an App for the operator, from the hub's registry; `None` when the hub does
/// not know GitHub or GitHub could not make one.
#[must_use]
pub fn github(registry: &Registry) -> Option<Registration> {
    registry
        .declaration(GITHUB)
        .and_then(acquisition::registration)
}

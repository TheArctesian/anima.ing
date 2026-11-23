//! Domain expiry check tests (CHK-009, D-29, P3-003); no test queries a real RDAP or IANA server.

mod bootstrap;
mod config;
mod egress;
mod observe;
pub(in crate::fleet::checks) mod registry;
mod reply;
pub(in crate::fleet::checks) mod server;
mod status;

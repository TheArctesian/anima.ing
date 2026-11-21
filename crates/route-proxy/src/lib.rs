//! The reverse proxy of the public names of applications (HOST-046, R-302; task H21;
//! Section 12.3 of `docs/proposals/hosted-applications-stage-2.md`).
//!
//! It is a process apart from the hub. It holds the certificates of the names it serves and
//! reads each request from the internet, so it gets only what [`wire`] gives: the table of the
//! names and streams that serve and a connection to a route of an application by its
//! identifier (SF-231). It cannot name a device, an address or a port. It serves
//! web routes, redirects, and streams of TCP and UDP (HOST-050, task H40), public or private.

pub mod door_header;
pub mod head;
pub mod hub;
pub mod limits;
pub mod plain;
pub mod relay;
pub mod serve;
pub mod settings;
pub mod streams;
pub mod tls;
pub mod wire;

#[cfg(test)]
mod tests;

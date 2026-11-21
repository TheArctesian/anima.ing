//! The streams of TCP and UDP (HOST-050, task H40; Section 16 of
//! `docs/proposals/hosted-applications-stage-2.md`): a port of the hub to the port of an
//! application, with no TLS of the hub and no look inside. Each connection or flow goes through
//! the one interface of the hub by the identifier of the application and the name of the route,
//! never by a device or a port, with the limits of each name and the cap of each client.

pub mod datagrams;
pub mod listeners;
mod tcp;
pub mod udp;

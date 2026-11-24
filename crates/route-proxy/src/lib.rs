//! The reverse proxy of the public names of applications (HOST-046, R-302).
//!
//! It reads each request from the internet, so it gets only what [`wire`] gives and cannot
//! name a device, an address or a port (SF-231).

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

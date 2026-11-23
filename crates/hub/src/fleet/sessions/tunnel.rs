//! The tunnel between the operator's client and guacd for RDP and VNC sessions (K4, GW-010
//! to GW-015, HUB-002).

mod arguments;
mod client;
mod decimal;
mod ending;
mod gateway;
mod guacd;
mod opening;
mod policy;
mod relay;
mod relaying;
mod route;
#[cfg(test)]
mod tests;

pub use route::tunnel_router;

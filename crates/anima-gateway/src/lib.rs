//! The Anima gateway: it speaks the Guacamole protocol to the hub as guacd does (spec Section 4.3, K4 Section 3).

pub mod frames;
pub mod handshake;
pub mod listening;
pub mod rdp;
pub mod screen;
pub mod session;
pub mod view;
pub mod vnc;

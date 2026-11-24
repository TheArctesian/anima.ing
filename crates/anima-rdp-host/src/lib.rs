//! The Apple Silicon RDP host carried by the Anima desktop application (MRDP-001).

#![cfg(all(target_os = "macos", target_arch = "aarch64"))]

mod capture;
mod input;
mod server;

pub use capture::{Permissions, permissions, request_permissions};
pub use server::{Host, Identity};

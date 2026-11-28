//! A process reference for the local Git read boundary (TH116).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub pid: u32,
    pub device: u64,
    pub inode: u64,
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
mod linux;
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub use linux::Process;

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
mod unsupported;
#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
pub use unsupported::Process;

//! Reads this computer and writes nothing (MET-002, CAP-028, CAP-032); parsing is in `daemon_core` (MET-016).

pub mod agent_sessions;
pub mod command;
pub mod container;
pub mod gpu;
mod hardware;
mod inventory;
mod linux;
mod macos;
pub mod mobile;
mod remote_access;
#[cfg(windows)]
mod windows;

use chrono::{DateTime, Utc};
use daemon_core::readings::Reading;
pub use hardware::hardware;
pub use inventory::inventory;
pub(crate) use remote_access::refresh as refresh_remote_access;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Macos,
    #[cfg(windows)]
    Windows,
}

impl Platform {
    #[must_use]
    pub const fn current() -> Self {
        #[cfg(windows)]
        {
            Self::Windows
        }
        #[cfg(unix)]
        if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Linux
        }
    }
}

/// In the container of a hosted hub a reading of the machine would show the server (HMD-071, HMD-074).
#[derive(Debug)]
pub enum Source {
    Machine(Platform),
    HostedHub(container::Container),
}

impl Source {
    /// The wall clock serves macOS, the monotonic one the container.
    pub async fn reading(&mut self, now: DateTime<Utc>, instant: std::time::Instant) -> Reading {
        match self {
            Self::Machine(platform) => reading(*platform, now).await,
            Self::HostedHub(container) => container.reading(instant).await,
        }
    }

    /// Never in a container, where a GPU would be one of the server.
    #[must_use]
    pub const fn has_gpus(&self) -> bool {
        matches!(self, Self::Machine(_))
    }
}

/// A reader that fails leaves its field empty, so the sample is missing and the daemon carries on.
pub async fn reading(platform: Platform, now: DateTime<Utc>) -> Reading {
    match platform {
        Platform::Linux => linux::reading().await,
        Platform::Macos => macos::reading(now).await,
        #[cfg(windows)]
        Platform::Windows => windows::reading().await,
    }
}

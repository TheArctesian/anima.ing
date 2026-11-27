#![cfg(target_os = "linux")]

mod command;
mod inventory;
mod mounts;
mod records;

pub use inventory::{VolumeObservation, inventory};
pub use mounts::mount_recorded;
pub use records::{MAX_VOLUMES, RECORD_NAME, Volume};

const PERSIST: &str = "/persist";
const VOLUME_MOUNTS: &str = "/persist/volumes";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Refused(&'static str),
    #[error("Could not access recorded data volumes. Check local storage.")]
    Io(#[from] std::io::Error),
    #[error("The recorded volume list or mount inventory is invalid.")]
    Json(#[from] serde_json::Error),
    #[error("The data volume command {0} failed. Check local storage.")]
    Command(&'static str),
}

type Result<T> = std::result::Result<T, Error>;

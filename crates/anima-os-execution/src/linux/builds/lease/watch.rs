//! Watchdog failure stops all leased units through their systemd dependency.
use super::{
    super::{lifecycle, store},
    expired_attempt,
};
use crate::linux::{Error, Result};
use std::{fs, time::Duration};
use uuid::Uuid;

pub(in crate::linux) async fn watch() -> Result<()> {
    let mut ticks = tokio::time::interval(Duration::from_secs(5));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        for (count, entry) in fs::read_dir(store::ROOT)?.enumerate() {
            if count >= 16384 {
                return Err(Error::Refused("Build lease inventory exceeded its bound."));
            }
            let name = entry?.file_name();
            if name == "locks" {
                continue;
            }
            let id = name
                .to_str()
                .and_then(|name| Uuid::parse_str(name).ok())
                .ok_or(Error::Refused("Invalid build lease inventory."))?;
            let attempt = match store::required(id) {
                Ok(attempt) => attempt,
                Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            let needs_stop = expired_attempt(&attempt)
                || (attempt.lease.is_some() && attempt.phase == "cancelling");
            if needs_stop && lifecycle::expire(id).await.is_err() {
                // A busy admission owner or uncertain runtime retains capacity until a retry proves termination.
                eprintln!("Build lease cancellation awaits runtime reconciliation.");
            }
        }
    }
}

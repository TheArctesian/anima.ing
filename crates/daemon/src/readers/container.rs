//! Only figures of the container of a hosted hub (HMD-071, HMD-072, HMD-074). It reads no `/proc`
//! file, runs no command and reads no GPU: in a container those can show the server.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use daemon_core::readings::{HubStorage, Reading, container};

const CGROUP: &str = "sys/fs/cgroup";

/// A walk of the data folder costs more than a read.
const MEASURE_EVERY: Duration = Duration::from_mins(1);

#[derive(Debug)]
pub struct Container {
    /// `/`; a test gives a folder that holds the files of a container and of a server.
    root: PathBuf,
    data: PathBuf,
    processors: u64,
    start: Option<(Instant, u64)>,
    measured: Option<(Instant, u64)>,
}

impl Container {
    #[must_use]
    pub const fn new(root: PathBuf, data: PathBuf, processors: u64) -> Self {
        Self {
            root,
            data,
            processors,
            start: None,
            measured: None,
        }
    }

    pub async fn reading(&mut self, now: Instant) -> Reading {
        let group = self.root.join(CGROUP);
        let file = |name: &str| read(group.join(name));
        let (memory_max, memory_current, memory_stat, cpu_max, cpu_stat) = tokio::join!(
            file("memory.max"),
            file("memory.current"),
            file("memory.stat"),
            file("cpu.max"),
            file("cpu.stat"),
        );
        let stat = memory_stat.unwrap_or_default();
        let memory = memory_max
            .zip(memory_current)
            .and_then(|(max, current)| container::memory(&max, &current, &stat));
        let used = cpu_stat.as_deref().and_then(container::cpu_usage_micros);
        let allowed = container::allowed_processors(&cpu_max.unwrap_or_default(), self.processors);
        let cpu = used.map(|used| {
            let (since, at_start) = *self.start.get_or_insert((now, used));
            let elapsed = u64::try_from(now.duration_since(since).as_micros()).unwrap_or(u64::MAX);
            container::cpu_ticks(used.saturating_sub(at_start), elapsed, allowed)
        });
        Reading {
            cpu,
            memory,
            hub_storage: self.hub_storage(now).await,
            ..Reading::default()
        }
    }

    async fn hub_storage(&mut self, now: Instant) -> Option<HubStorage> {
        let due = self
            .measured
            .is_none_or(|(at, _)| now.duration_since(at) >= MEASURE_EVERY);
        if due {
            let folder = self.data.clone();
            let bytes = tokio::task::spawn_blocking(move || bytes_under(&folder)).await;
            self.measured = bytes.ok().flatten().map(|bytes| (now, bytes));
            if self.measured.is_none() {
                tracing::debug!("the data folder of the hub could not be measured");
            }
        }
        let (_, bytes) = self.measured?;
        Some(HubStorage {
            folder: self.data.to_str()?.to_owned(),
            bytes,
        })
    }
}

async fn read(path: PathBuf) -> Option<Vec<u8>> {
    tokio::fs::read(path).await.ok()
}

/// Counts as `du -sb` does; a link is not followed, so nothing outside the folder counts.
fn bytes_under(folder: &Path) -> Option<u64> {
    let mut total = 0_u64;
    let mut folders = vec![folder.to_path_buf()];
    let mut first = true;
    while let Some(next) = folders.pop() {
        let entries = match std::fs::read_dir(&next) {
            Ok(entries) => entries,
            Err(_) if first => return None,
            Err(_) => continue,
        };
        first = false;
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                folders.push(entry.path());
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Some(total)
}

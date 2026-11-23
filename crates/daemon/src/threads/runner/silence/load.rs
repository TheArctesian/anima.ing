//! The load of the computer, which sets how long a program has to answer `--version`.

use std::time::Duration;

use super::{PROBE_CAP, PROBE_WAIT};

/// The load of the computer: the average of the last minute, and its processor cores.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Load {
    pub one: f64,
    pub cores: usize,
}

impl Load {
    /// More programs want to run than the computer has cores.
    #[must_use]
    pub fn busy(self) -> bool {
        self.one > f64::from(u32::try_from(self.cores).unwrap_or(u32::MAX))
    }
}

/// The time a program has to answer `--version` under `load`: [`PROBE_WAIT`] for each core's
/// worth of load, at least once and at most [`PROBE_CAP`]. A daemon of background priority
/// gets little time on a busy computer (measured 2026-11-22: 0.1 to 5.8 s at a load of 146 on
/// 16 cores, for a program that answers in 0.01 s), so the time follows the load.
#[must_use]
pub fn probe_wait(load: Option<Load>) -> Duration {
    let Some(load) = load else {
        return PROBE_WAIT;
    };
    let cores = f64::from(u32::try_from(load.cores.max(1)).unwrap_or(u32::MAX));
    let factor = (load.one / cores).clamp(1.0, PROBE_CAP.as_secs_f64() / PROBE_WAIT.as_secs_f64());
    PROBE_WAIT.mul_f64(if factor.is_finite() { factor } else { 1.0 })
}

/// The load of this computer: `vm.loadavg` of `sysctl` on macOS, `/proc/loadavg` on Linux;
/// `None` when it cannot be read in 2 seconds.
pub async fn load() -> Option<Load> {
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let read = async {
        if cfg!(target_os = "macos") {
            let output = tokio::process::Command::new("/usr/sbin/sysctl")
                .args(["-n", "vm.loadavg"])
                .env_clear()
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .output()
                .await
                .ok()?;
            Some(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            tokio::fs::read_to_string("/proc/loadavg").await.ok()
        }
    };
    let text = tokio::time::timeout(Duration::from_secs(2), read)
        .await
        .ok()??;
    Some(Load {
        one: first_load(&text)?,
        cores,
    })
}

/// The average of the last minute in `{ 1.50 1.20 1.00 }` (macOS) or `1.50 1.20 1.00 2/300 9`
/// (Linux).
pub(super) fn first_load(text: &str) -> Option<f64> {
    text.split_whitespace()
        .find(|word| *word != "{")?
        .parse()
        .ok()
        .filter(|load: &f64| load.is_finite() && *load >= 0.0)
}

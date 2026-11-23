//! The wait for processes that got `SIGKILL` to be gone (THR-049).

use std::collections::BTreeSet;
use std::time::Duration;

use super::{Ended, alive, ps_output};

/// The longest wait for processes that got `SIGKILL` to be gone. On a busy computer (a load
/// above 100) a killed process can take longer than a tenth of a second to end and be reaped,
/// and a stop that looked once then said it could not prove the end (2026-11-22).
const SETTLE: Duration = Duration::from_secs(5);

/// How often the wait asks `ps` which of the processes left are zombies.
const ASK_ZOMBIES: Duration = Duration::from_millis(500);

/// What an end of processes signalled, before the wait for them to be gone. The ends of one
/// stop, or of each thread at the end of the holder, add up ([`Killed::and`]) and wait one time.
#[derive(Debug)]
pub struct Killed {
    /// How many processes got a signal to end.
    pub count: u32,
    /// Whether the end looked at everything it had to: false when a table or an environment
    /// could not be read.
    pub sure: bool,
    /// The processes that got `SIGKILL`.
    pub pids: BTreeSet<u32>,
}

impl Killed {
    /// Nothing to end: the start of a sum.
    #[must_use]
    pub const fn nothing() -> Self {
        Self {
            count: 0,
            sure: true,
            pids: BTreeSet::new(),
        }
    }

    /// This end and `other` as one.
    #[must_use]
    pub fn and(mut self, other: Self) -> Self {
        self.count = self.count.saturating_add(other.count);
        self.sure = self.sure && other.sure;
        self.pids.extend(other.pids);
        self
    }

    /// The end, when `left` are the processes that still run after the wait.
    #[must_use]
    pub fn proved(&self, left: &BTreeSet<u32>) -> Ended {
        Ended {
            count: self.count,
            all: self.sure && self.pids.is_disjoint(left),
        }
    }

    /// The end after its own wait ([`left_running`]).
    pub async fn settled(self) -> Ended {
        let left = left_running(&self.pids).await;
        self.proved(&left)
    }
}

/// Which of `pids` still run after they got `SIGKILL`: each is looked at every 50 ms until none
/// runs or [`SETTLE`] passed. Every [`ASK_ZOMBIES`] and at the end, one that the system still
/// lists is asked its state, so that a zombie (it ended; its parent has not reaped it yet)
/// counts as ended on a Mac too, and does not hold the stop for the whole wait.
pub async fn left_running(pids: &BTreeSet<u32>) -> BTreeSet<u32> {
    let start = tokio::time::Instant::now();
    let mut asked = start;
    loop {
        let left: Vec<u32> = pids.iter().copied().filter(|pid| alive(*pid)).collect();
        if left.is_empty() {
            return BTreeSet::new();
        }
        let now = tokio::time::Instant::now();
        if now >= start + SETTLE {
            return not_zombies(&left, ASK_ZOMBIES).await;
        }
        if now >= asked + ASK_ZOMBIES {
            asked = now;
            if not_zombies(&left, start + SETTLE - now).await.is_empty() {
                return BTreeSet::new();
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Which of `pids` are not zombies, as `ps` says it within `budget`; each one counts when `ps`
/// cannot say, so that a slow `ps` never stretches the wait far past [`SETTLE`].
async fn not_zombies(pids: &[u32], budget: Duration) -> BTreeSet<u32> {
    let list = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let zombies = tokio::time::timeout(budget, ps_output(&["-o", "pid=,stat=", "-p", &list]))
        .await
        .ok()
        .flatten()
        .map_or_else(BTreeSet::new, |output| {
            zombies_in(&String::from_utf8_lossy(&output))
        });
    pids.iter()
        .copied()
        .filter(|pid| alive(*pid) && !zombies.contains(pid))
        .collect()
}

/// The processes of `ps -o pid=,stat=` whose state is `Z`.
#[must_use]
pub fn zombies_in(table: &str) -> BTreeSet<u32> {
    table
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse().ok()?;
            fields.next()?.starts_with('Z').then_some(pid)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn thr_049_a_killed_child_that_is_not_reaped_counts_as_ended_before_the_wait_ends() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .expect("sleep starts");
        let pid = child.id();
        child.kill().expect("killed");
        let begun = std::time::Instant::now();
        assert_eq!(
            left_running(&BTreeSet::from([pid])).await,
            BTreeSet::new(),
            "a zombie ended"
        );
        assert!(
            begun.elapsed() < SETTLE,
            "the zombie did not hold the whole wait"
        );
        let _ = child.wait();
    }
}

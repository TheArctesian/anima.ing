//! The wait for processes that got `SIGKILL` to be gone (THR-049).

use std::collections::BTreeSet;
use std::time::Duration;

use super::{alive, ps_output};

/// The longest wait for processes that got `SIGKILL` to be gone. On a busy computer (a load
/// above 100) a killed process can take longer than a tenth of a second to end and be reaped,
/// and a stop that looked once then said it could not prove the end (2026-11-22).
const SETTLE: Duration = Duration::from_secs(5);

/// How often the wait asks `ps` which of the processes left are zombies.
const ASK_ZOMBIES: Duration = Duration::from_millis(500);

/// How many of `pids` still run after they got `SIGKILL`: each is looked at every 50 ms until
/// none runs or [`SETTLE`] passed. Every [`ASK_ZOMBIES`] and at the end, one that the system
/// still lists is asked its state, so that a zombie (it ended; its parent has not reaped it yet)
/// counts as ended on a Mac too, and does not hold the stop for the whole wait.
pub async fn left_running(pids: &BTreeSet<u32>) -> usize {
    let start = tokio::time::Instant::now();
    let mut asked = start;
    loop {
        let left: Vec<u32> = pids.iter().copied().filter(|pid| alive(*pid)).collect();
        if left.is_empty() {
            return 0;
        }
        let now = tokio::time::Instant::now();
        if now >= start + SETTLE {
            return not_zombies(&left, ASK_ZOMBIES).await;
        }
        if now >= asked + ASK_ZOMBIES {
            asked = now;
            if not_zombies(&left, start + SETTLE - now).await == 0 {
                return 0;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// How many of `pids` are not zombies, as `ps` says it within `budget`; each one counts when
/// `ps` cannot say, so that a slow `ps` never stretches the wait far past [`SETTLE`].
async fn not_zombies(pids: &[u32], budget: Duration) -> usize {
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
        .filter(|pid| alive(**pid) && !zombies.contains(pid))
        .count()
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
            0,
            "a zombie ended"
        );
        assert!(
            begun.elapsed() < SETTLE,
            "the zombie did not hold the whole wait"
        );
        let _ = child.wait();
    }
}

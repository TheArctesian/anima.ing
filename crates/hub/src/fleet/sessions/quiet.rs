//! One side's silence on a tunnel (K4 Sections 4, 8.4 and 9). The caller passes the time
//! (CQ-021) as a `tokio::time::Instant`, so tests with paused time drive it.

use std::time::Duration;

use tokio::time::Instant;

#[derive(Clone, Copy)]
pub(in crate::fleet::sessions) struct Quiet {
    wait: Duration,
    due: Instant,
}

impl Quiet {
    pub(in crate::fleet::sessions) fn new(wait: Duration, now: Instant) -> Self {
        Self {
            wait,
            due: now + wait,
        }
    }

    pub(in crate::fleet::sessions) fn restart(&mut self, now: Instant) {
        self.due = now + self.wait;
    }

    pub(in crate::fleet::sessions) const fn due(&self) -> Instant {
        self.due
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::Instant;

    use super::Quiet;

    const WAIT: Duration = Duration::from_secs(5);

    #[test]
    fn k4_4_a_silence_comes_due_its_wait_after_it_starts() {
        let start = Instant::now();

        assert_eq!(Quiet::new(WAIT, start).due(), start + WAIT);
    }

    #[test]
    fn k4_4_a_turn_restarts_the_silence_from_the_turn_not_from_the_start() {
        let start = Instant::now();
        let mut quiet = Quiet::new(WAIT, start);
        let turn = start + Duration::from_secs(3);

        quiet.restart(turn);

        assert_eq!(quiet.due(), turn + WAIT);
    }
}

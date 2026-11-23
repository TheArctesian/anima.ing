//! The waits and the counts of the supervision of a server (Section 9, CAG-005, CAG-007). Pure:
//! the time is a parameter in whole seconds of a clock that only goes forward.

pub const WAITS: [u64; 9] = [1, 2, 4, 8, 16, 32, 64, 128, 256];
pub const LAST_WAIT: u64 = 300;

const HOUR: u64 = 3_600;
const DAY: u64 = 86_400;
pub const LIMIT_HOUR: usize = 5;
pub const LIMIT_DAY: usize = 20;
pub const CLEAR_AFTER: u64 = 600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    RestartAt(u64),
    Failed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Supervision {
    ends: Vec<u64>,
    failed: bool,
    clear_pending: bool,
    ready_since: Option<u64>,
}

impl Supervision {
    #[must_use]
    pub fn wait(index: usize) -> u64 {
        WAITS.get(index).copied().unwrap_or(LAST_WAIT)
    }

    pub fn started(&mut self) {
        self.failed = false;
        self.clear_pending = true;
        self.ready_since = None;
    }

    pub fn ready(&mut self, now: u64) {
        self.ready_since.get_or_insert(now);
        self.tick(now);
    }

    pub fn tick(&mut self, now: u64) {
        let ready_long = self
            .ready_since
            .is_some_and(|since| now.saturating_sub(since) >= CLEAR_AFTER);
        if self.clear_pending && ready_long {
            self.ends.clear();
            self.clear_pending = false;
        }
    }

    /// An end that no item asked for.
    pub fn ended(&mut self, now: u64) -> Next {
        self.ready_since = None;
        self.ends.retain(|at| now.saturating_sub(*at) < DAY);
        self.ends.push(now);
        let in_hour = self
            .ends
            .iter()
            .filter(|at| now.saturating_sub(**at) < HOUR)
            .count();
        if self.failed || in_hour >= LIMIT_HOUR || self.ends.len() >= LIMIT_DAY {
            self.failed = true;
            return Next::Failed;
        }
        Next::RestartAt(now + Self::wait(self.ends.len() - 1))
    }

    #[must_use]
    pub fn restarts(&self) -> usize {
        self.ends.len()
    }

    #[must_use]
    pub const fn is_failed(&self) -> bool {
        self.failed
    }
}

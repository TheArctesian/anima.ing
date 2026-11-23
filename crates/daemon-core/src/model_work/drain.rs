//! The drain of `model_drain` (CAP-013): the daemon sends a server no new request and stops no
//! server.

/// The longest wait of a drain (CAP-013).
pub const DRAIN_LIMIT_SECONDS: u64 = 60;

/// A drain that started at a second of the caller's clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drain {
    deadline: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Wait,
    /// The drain is over; `left` requests were still in progress at the deadline.
    Done {
        left: usize,
    },
}

impl Drain {
    /// A drain of `seconds` from `now`; a longer wait than the limit is cut to the limit.
    #[must_use]
    pub fn new(seconds: u32, now: u64) -> Self {
        Self {
            deadline: now.saturating_add(u64::from(seconds).min(DRAIN_LIMIT_SECONDS)),
        }
    }

    #[must_use]
    pub const fn step(&self, in_progress: usize, now: u64) -> Step {
        if in_progress == 0 || now >= self.deadline {
            Step::Done { left: in_progress }
        } else {
            Step::Wait
        }
    }
}

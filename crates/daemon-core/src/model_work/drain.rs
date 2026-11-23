//! The drain of `model_drain` (Section 2, CAP-013): the daemon sends no new request to a
//! server and waits for the requests in progress for at most `seconds`, never more than
//! [`DRAIN_LIMIT_SECONDS`]. It stops no server. Pure: the caller gives the count and the clock.

/// The longest wait of a drain (CAP-013).
pub const DRAIN_LIMIT_SECONDS: u64 = 60;

/// A drain that started at a second of the caller's clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drain {
    deadline: u64,
}

/// What the drain does at one look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Requests are in progress and the deadline has not come.
    Wait,
    /// The drain is over; `left` requests were still in progress at the deadline.
    Done { left: usize },
}

impl Drain {
    /// A drain of `seconds` from `now`; a longer wait than the limit is cut to the limit.
    #[must_use]
    pub fn new(seconds: u32, now: u64) -> Self {
        Self {
            deadline: now.saturating_add(u64::from(seconds).min(DRAIN_LIMIT_SECONDS)),
        }
    }

    /// The step at `now` with `in_progress` requests.
    #[must_use]
    pub const fn step(&self, in_progress: usize, now: u64) -> Step {
        if in_progress == 0 || now >= self.deadline {
            Step::Done { left: in_progress }
        } else {
            Step::Wait
        }
    }
}

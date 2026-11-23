//! When the next frame may go to the client: while the client is behind, a frame waits, so the next one holds only the newest pixels.
//!
//! The caller gives the time, counted from the start of the session.

use std::collections::VecDeque;
use std::time::Duration;

/// The limits of one session's frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pace {
    /// The shortest time from one frame to the next: the frame rate limit.
    pub least_interval: Duration,
    /// How many frames may be on their way to the client and not yet answered.
    pub most_unanswered: usize,
}

impl Default for Pace {
    /// Two frames on their way, one drawn and one behind it, so a fast client never waits for the gateway.
    fn default() -> Self {
        Self {
            least_interval: Duration::from_millis(33),
            most_unanswered: 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Now,
    At(Duration),
    /// When the client answers a frame it has.
    AfterAnswer,
}

/// The frames of one session that the client has not answered yet.
#[derive(Debug)]
pub struct Pacing {
    pace: Pace,
    /// The timestamps of the unanswered frames, oldest first.
    unanswered: VecDeque<u64>,
    /// When the latest frame was sent, and its timestamp.
    latest: Option<(Duration, u64)>,
}

impl Pacing {
    /// A limit of no unanswered frames would send nothing; it counts as one.
    #[must_use]
    pub fn new(pace: Pace) -> Self {
        let pace = Pace {
            most_unanswered: pace.most_unanswered.max(1),
            ..pace
        };
        Self {
            pace,
            unanswered: VecDeque::new(),
            latest: None,
        }
    }

    #[must_use]
    pub fn next(&self, now: Duration) -> Next {
        if self.unanswered.len() >= self.pace.most_unanswered {
            return Next::AfterAnswer;
        }
        let earliest = self
            .latest
            .map(|(sent, _)| sent.saturating_add(self.pace.least_interval));
        match earliest {
            Some(earliest) if now < earliest => Next::At(earliest),
            _ => Next::Now,
        }
    }

    /// Notes a frame sent at `now` and gives the timestamp for its `sync`; `None` when no frame may be sent now.
    /// Each timestamp is above 0 and the one before: the client starts at 0 and answers only a
    /// timestamp that differs from its own (guacamole-common-js `Guacamole.Client`).
    pub fn send(&mut self, now: Duration) -> Option<u64> {
        if self.next(now) != Next::Now {
            return None;
        }
        let millis = u64::try_from(now.as_millis()).unwrap_or(u64::MAX);
        let after = self.latest.map_or(0, |(_, before)| before);
        let timestamp = millis.max(after.saturating_add(1));
        self.latest = Some((now, timestamp));
        self.unanswered.push_back(timestamp);
        Some(timestamp)
    }

    /// The client's `sync` with `timestamp`: it has drawn that frame and every frame
    /// before it. Text that is not a timestamp answers nothing.
    pub fn answered(&mut self, timestamp: &str) {
        let Ok(drawn) = timestamp.parse::<u64>() else {
            return;
        };
        while self.unanswered.front().is_some_and(|sent| *sent <= drawn) {
            self.unanswered.pop_front();
        }
    }
}

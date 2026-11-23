//! When a tunnel's client pings or gives up (K4 Sections 8.4 and 9); every call takes the time (CQ-021).

use std::time::Duration;

use tokio::time::Instant;

pub const PING_AFTER: Duration = Duration::from_secs(5);
pub const SILENCE_LIMIT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Due {
    Ping,
    Lost,
}

#[derive(Clone, Copy, Debug)]
pub struct Pace {
    sent: Instant,
    heard: Instant,
}

impl Pace {
    #[must_use]
    pub const fn new(now: Instant) -> Self {
        Self {
            sent: now,
            heard: now,
        }
    }

    pub fn sent(&mut self, now: Instant) {
        self.sent = self.sent.max(now);
    }

    pub fn heard(&mut self, now: Instant) {
        self.heard = self.heard.max(now);
    }

    #[must_use]
    pub fn due(&self, now: Instant) -> Option<Due> {
        if now >= self.heard + SILENCE_LIMIT {
            Some(Due::Lost)
        } else if now >= self.sent + PING_AFTER {
            Some(Due::Ping)
        } else {
            None
        }
    }

    #[must_use]
    pub fn next(&self) -> Instant {
        (self.sent + PING_AFTER).min(self.heard + SILENCE_LIMIT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(start: Instant, seconds: u64) -> Instant {
        start + Duration::from_secs(seconds)
    }

    #[test]
    fn k4_8_4_a_ping_follows_five_quiet_seconds() {
        let start = Instant::now();
        let mut pace = Pace::new(start);
        assert_eq!(pace.due(at(start, 4)), None);
        assert_eq!(pace.next(), at(start, 5));
        assert_eq!(pace.due(at(start, 5)), Some(Due::Ping));
        pace.sent(at(start, 5));
        pace.heard(at(start, 5));
        assert_eq!(pace.due(at(start, 9)), None);
        assert_eq!(pace.next(), at(start, 10));
    }

    #[test]
    fn k4_8_4_fifteen_silent_seconds_lose_the_connection_even_while_pinging() {
        let start = Instant::now();
        let mut pace = Pace::new(start);
        for second in [5, 10] {
            assert_eq!(pace.due(at(start, second)), Some(Due::Ping));
            pace.sent(at(start, second));
        }
        assert_eq!(pace.due(at(start, 14)), None);
        assert_eq!(pace.next(), at(start, 15));
        assert_eq!(pace.due(at(start, 15)), Some(Due::Lost));
    }

    #[test]
    fn k4_8_4_time_never_runs_backwards_for_the_pace() {
        let start = Instant::now();
        let mut pace = Pace::new(at(start, 10));
        pace.sent(start);
        pace.heard(start);
        assert_eq!(pace.next(), at(start, 15));
    }
}

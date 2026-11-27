//! Whether the phone's link starts again when the app comes back to the screen (MOB-036). It takes the time as a
//! value and does no I/O; `mobile.rs` gives it the window events and `phone_link` does the restart.

use std::{
    sync::{Mutex, PoisonError},
    time::{Duration, SystemTime},
};

/// iOS suspends an app a few seconds after it leaves the screen, and the link's sockets and tunnel are stale after
/// that, although the link still reports that it runs. A shorter look at another app keeps the link, so a live
/// session does not drop.
pub const SUSPENDED_AFTER: Duration = Duration::from_secs(10);

/// When the app last left the screen. A wall clock, because the uptime clock stops while the phone sleeps.
#[derive(Default)]
pub struct Away(Mutex<Option<SystemTime>>);

impl Away {
    /// The app is about to leave the screen; a later departure replaces an earlier one that did not go to the background.
    pub fn left(&self, at: SystemTime) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(at);
    }

    /// Whether the link must start again now that the app is back; each departure answers once. A clock that moved
    /// back says nothing about the time away, so the link starts again.
    pub fn back(&self, at: SystemTime) -> bool {
        let left = self.0.lock().unwrap_or_else(PoisonError::into_inner).take();
        left.is_some_and(
            |left| !matches!(at.duration_since(left), Ok(away) if away < SUSPENDED_AFTER),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{Away, SUSPENDED_AFTER};

    fn left_at(at: SystemTime) -> Away {
        let away = Away::default();
        away.left(at);
        away
    }

    #[test]
    fn mob_036_a_short_look_at_another_app_keeps_the_link() {
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        assert!(!left_at(at).back(at + SUSPENDED_AFTER - Duration::from_millis(1)));
    }

    #[test]
    fn mob_036_a_return_after_ios_suspended_the_app_starts_the_link_again() {
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        assert!(left_at(at).back(at + SUSPENDED_AFTER));
        assert!(left_at(at).back(at + Duration::from_secs(3_600)));
    }

    #[test]
    fn mob_036_the_first_return_to_the_screen_at_start_and_a_second_return_start_nothing() {
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        assert!(!Away::default().back(at));
        let away = left_at(at);
        assert!(away.back(at + SUSPENDED_AFTER));
        assert!(!away.back(at + SUSPENDED_AFTER * 2));
    }

    #[test]
    fn mob_036_the_time_away_counts_from_the_last_departure() {
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let away = left_at(at);
        away.left(at + Duration::from_secs(60));
        assert!(!away.back(at + Duration::from_secs(62)));
    }

    #[test]
    fn mob_036_a_clock_that_moved_back_starts_the_link_again() {
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        assert!(left_at(at).back(at - Duration::from_secs(5)));
    }
}

//! The push rhythm and the action for each answer (K8 Sections 6 and 7).

use std::time::Duration;

use daemon_core::points::BATCH_POINTS;
use daemon_core::wire::RefusalCode;

use crate::hub::answer::Answer;

/// The time between pushes while the hub answers well (MET-014).
pub const RHYTHM: Duration = Duration::from_secs(10);

/// Short, so a push that ran to its timeout is retried inside the hub's stale bound (MET-018, R-312).
pub const BACKOFF_START: Duration = Duration::from_secs(2);

pub const BACKOFF_MAX: Duration = Duration::from_secs(60);

/// How far a wait after a failure may move from its step, either way (K8 Section 7).
pub const JITTER: f64 = 0.2;

/// How long a refused token waits before the daemon tries it again (K8 Section 6).
pub const REFUSED_RETRY: Duration = Duration::from_mins(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Remove,
    /// The batch can never be accepted.
    Drop(RefusalCode),
    Shrink,
    /// Stop pushing; sampling and buffering go on.
    Stop(RefusalCode),
    Wait,
}

#[must_use]
pub const fn action<T>(answer: &Answer<T>) -> Action {
    match answer {
        Answer::Accepted(_) => Action::Remove,
        Answer::Refused(code) => match code {
            RefusalCode::Malformed | RefusalCode::OutOfRange => Action::Drop(*code),
            RefusalCode::TooLarge => Action::Shrink,
            RefusalCode::TokenRefused | RefusalCode::WrongDevice => Action::Stop(*code),
            // These codes are not push answers (K10 and others); treat any such as a wait.
            RefusalCode::EnrolmentRefused
            | RefusalCode::NoKey
            | RefusalCode::NoRelease
            | RefusalCode::NotHolder
            | RefusalCode::WorkOff
            | RefusalCode::SecretRefused
            | RefusalCode::Unavailable => Action::Wait,
        },
        Answer::Failed(_) => Action::Wait,
    }
}

#[must_use]
pub const fn smaller(sent: usize) -> Option<usize> {
    if sent > 1 { Some(sent / 2) } else { None }
}

#[must_use]
pub fn grown(limit: usize) -> usize {
    limit.saturating_mul(2).min(BATCH_POINTS)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Backoff {
    last: Option<Duration>,
}

impl Backoff {
    /// Spread by up to [`JITTER`] either way, so daemons that failed together do not retry together.
    pub fn failed(&mut self, spread: f64) -> Duration {
        let step = self
            .last
            .map_or(BACKOFF_START, |last| (last * 2).min(BACKOFF_MAX));
        self.last = Some(step);
        step.mul_f64(2.0f64.mul_add(JITTER * spread.clamp(0.0, 1.0), 1.0 - JITTER))
    }

    pub const fn recovered(&mut self) {
        self.last = None;
    }

    #[must_use]
    pub const fn is_failing(&self) -> bool {
        self.last.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::answer::Cause;

    #[test]
    fn k8_6_each_answer_maps_to_its_action() {
        let refused = |code| action(&Answer::<()>::Refused(code));
        assert_eq!(action(&Answer::Accepted(())), Action::Remove);
        assert_eq!(
            refused(RefusalCode::Malformed),
            Action::Drop(RefusalCode::Malformed)
        );
        assert_eq!(
            refused(RefusalCode::OutOfRange),
            Action::Drop(RefusalCode::OutOfRange)
        );
        assert_eq!(refused(RefusalCode::TooLarge), Action::Shrink);
        assert_eq!(
            refused(RefusalCode::TokenRefused),
            Action::Stop(RefusalCode::TokenRefused)
        );
        assert_eq!(
            refused(RefusalCode::WrongDevice),
            Action::Stop(RefusalCode::WrongDevice)
        );
        assert_eq!(refused(RefusalCode::Unavailable), Action::Wait);
        for cause in [Cause::Network, Cause::Timeout, Cause::Status(503)] {
            assert_eq!(action(&Answer::<()>::Failed(cause)), Action::Wait);
        }
    }

    #[test]
    fn k8_7_the_backoff_doubles_from_2_to_60_seconds_and_resets() {
        let mut backoff = Backoff::default();
        let waits: Vec<u64> = (0..8).map(|_| backoff.failed(0.5).as_secs()).collect();
        assert_eq!(waits, [2, 4, 8, 16, 32, 60, 60, 60]);
        assert!(backoff.is_failing());
        backoff.recovered();
        assert!(!backoff.is_failing());
        assert_eq!(backoff.failed(0.5).as_secs(), 2);
    }

    #[test]
    fn met_018_one_timed_out_push_is_retried_inside_the_stale_bound() {
        let stale = RHYTHM * 3;
        let latest_retry = Backoff::default().failed(1.0);
        assert!(RHYTHM + crate::hub::TIMEOUT + latest_retry < stale);
    }

    #[test]
    fn k8_7_each_wait_is_spread_20_percent_either_way_around_its_step() {
        let waits = |spread: f64| -> Vec<u128> {
            let mut backoff = Backoff::default();
            (0..7).map(|_| backoff.failed(spread).as_millis()).collect()
        };
        assert_eq!(
            waits(0.0),
            [1_600, 3_200, 6_400, 12_800, 25_600, 48_000, 48_000]
        );
        assert_eq!(
            waits(1.0),
            [2_400, 4_800, 9_600, 19_200, 38_400, 72_000, 72_000]
        );
        assert_eq!(
            waits(-3.0),
            waits(0.0),
            "a spread past its range is held to it"
        );
        assert_eq!(waits(7.0), waits(1.0));
    }

    #[test]
    fn k8_6_too_large_halves_until_one_point_which_is_dropped() {
        assert_eq!(smaller(60), Some(30));
        assert_eq!(smaller(3), Some(1));
        assert_eq!(smaller(1), None);
        assert_eq!(grown(1), 2);
        assert_eq!(grown(40), 60);
    }
}

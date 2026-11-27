//! Admission rate for fresh person intents. A signed-in hub has one operator (OD-2).

use std::{
    sync::{Mutex, PoisonError},
    time::{Duration, Instant},
};

use uuid::Uuid;

use super::Error;

#[derive(Clone, Copy)]
pub(super) enum Lane {
    Prompt,
    Answer,
    Stop,
}

#[derive(Default)]
pub(crate) struct MutationPace(Mutex<OperatorPace>);

#[derive(Default)]
struct OperatorPace {
    account: Uuid,
    prompt: Bucket,
    answer: Bucket,
    stop: Bucket,
}

#[derive(Default)]
struct Bucket {
    next: Option<Instant>,
}

impl MutationPace {
    pub(super) fn admit(&self, account: Uuid, lane: Lane) -> Result<(), Error> {
        let mut pace = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if pace.account != account {
            *pace = OperatorPace {
                account,
                ..OperatorPace::default()
            };
        }
        pace.admit(lane, Instant::now())
            .then_some(())
            .ok_or(Error::Refused("RATE_LIMITED"))
    }
}

impl OperatorPace {
    fn admit(&mut self, lane: Lane, now: Instant) -> bool {
        let (bucket, refill, burst) = match lane {
            Lane::Prompt => (&mut self.prompt, Duration::from_secs(1), 40),
            Lane::Answer => (&mut self.answer, Duration::from_millis(250), 16),
            Lane::Stop => (&mut self.stop, Duration::from_millis(250), 8),
        };
        bucket.spend(now, refill, burst)
    }
}

impl Bucket {
    fn spend(&mut self, now: Instant, refill: Duration, burst: u32) -> bool {
        let Some(next) = self.next.unwrap_or(now).max(now).checked_add(refill) else {
            return false;
        };
        if next.saturating_duration_since(now) > refill * burst {
            return false;
        }
        self.next = Some(next);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_flood_does_not_spend_answer_or_stop_budgets() {
        let mut pace = OperatorPace::default();
        let now = Instant::now();
        assert!((0..40).all(|_| pace.admit(Lane::Prompt, now)));
        assert!(!pace.admit(Lane::Prompt, now));
        assert!((0..16).all(|_| pace.admit(Lane::Answer, now)));
        assert!(!pace.admit(Lane::Answer, now));
        assert!((0..8).all(|_| pace.admit(Lane::Stop, now)));
        assert!(!pace.admit(Lane::Stop, now));
        assert!(pace.admit(Lane::Stop, now + Duration::from_millis(250)));
        assert!(!pace.admit(Lane::Prompt, now + Duration::from_millis(250)));
        let later = now + Duration::from_secs(1);
        assert!(pace.admit(Lane::Prompt, later));
        assert!(!pace.admit(Lane::Prompt, later));
    }
}

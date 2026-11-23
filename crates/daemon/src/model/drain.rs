//! `model_drain` at the machine (Section 2, CAP-013): the I/O edge of
//! [`daemon_core::model_work::drain`]. It stops no server and starts nothing.

use std::time::Duration;

use tokio::time::Instant;

use daemon_core::model_work::drain::{Drain, Step};

pub async fn drain(seconds: u32, in_progress: impl Fn() -> usize) -> usize {
    let clock = Instant::now();
    let drain = Drain::new(seconds, 0);
    loop {
        match drain.step(in_progress(), clock.elapsed().as_secs()) {
            Step::Done { left } => return left,
            Step::Wait => tokio::time::sleep(Duration::from_millis(250)).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    #[tokio::test(start_paused = true)]
    async fn a_drain_with_requests_left_ends_at_its_deadline_with_the_count() {
        let start = tokio::time::Instant::now();
        let left = super::drain(3, || 2).await;
        assert_eq!(left, 2);
        let waited = start.elapsed();
        assert!(waited >= Duration::from_secs(3) && waited < Duration::from_secs(4));
    }
}

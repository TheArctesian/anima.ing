//! A round's collections as tasks, at most a fixed number at a time (D-11).

use std::vec;

use tokio::task::{JoinError, JoinSet};

/// Dropping it aborts the jobs still running.
pub(super) struct FanOut<Job: Future> {
    waiting: vec::IntoIter<Job>,
    running: JoinSet<Job::Output>,
}

impl<Job> FanOut<Job>
where
    Job: Future + Send + 'static,
    Job::Output: Send + 'static,
{
    pub(super) fn new(jobs: Vec<Job>, at_once: usize) -> Self {
        let mut waiting = jobs.into_iter();
        let mut running = JoinSet::new();
        for job in waiting.by_ref().take(at_once.max(1)) {
            running.spawn(job);
        }
        Self { waiting, running }
    }

    pub(super) async fn next(&mut self) -> Option<Result<Job::Output, JoinError>> {
        let joined = self.running.join_next().await?;
        if let Some(job) = self.waiting.next() {
            self.running.spawn(job);
        }
        Some(joined)
    }
}

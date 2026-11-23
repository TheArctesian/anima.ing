//! One install at a time, stopped when the app exits, so no daemon run outlives the app (MET-019).

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use super::error::DaemonError;

const STOP_PATIENCE: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(10);

#[derive(Debug, Default)]
pub struct Installing {
    busy: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

#[derive(Debug)]
pub struct Claim {
    busy: Arc<AtomicBool>,
    pub stop: Arc<AtomicBool>,
}

impl Installing {
    /// # Errors
    /// [`DaemonError::Busy`] while another install holds it.
    pub fn claim(&self) -> Result<Claim, DaemonError> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Claim {
                busy: Arc::clone(&self.busy),
                stop: Arc::clone(&self.stop),
            })
            .map_err(|_| DaemonError::Busy)
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        let end = Instant::now() + STOP_PATIENCE;
        while self.busy.load(Ordering::Acquire) && Instant::now() < end {
            thread::sleep(POLL);
        }
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        self.busy.store(false, Ordering::Release);
    }
}

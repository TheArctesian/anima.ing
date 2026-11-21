//! A Keychain read that cannot hold the daemon for ever (R-322). macOS asks the
//! person before a program that is not on the access list of an item reads it; an ad hoc
//! signed build is a new program at each build, so its read can wait for a click that never
//! comes, and the OS call has no timeout.
//!
//! The read runs on a thread of its own and the caller waits [`DEADLINE`] at most. The blocked
//! OS call cannot be cancelled, so a read that passed its deadline is left to end; the next
//! read of the same item waits for that one and starts no second thread. So one read is in
//! flight for each item, and the answer that the person gives late is the answer of the next
//! read. While the daemon waits, the file [`MARK`] in its folder tells the app (which cannot
//! ask the hub for it: without the token the daemon cannot reach the hub).

use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use crate::config::home::Home;

/// How long a caller waits for one Keychain read. An allowed read ends in milliseconds, and
/// the first unlock of the login keychain in a second or two; a prompt that nobody answers
/// never ends. 5 s is well above the first and keeps `install` far below the 30 s of one
/// step and the deadline of the app (R-322).
pub const DEADLINE: Duration = Duration::from_secs(5);

/// The file in the daemon's folder that is there while the daemon waits for the person to
/// allow it in the Keychain. Empty: its presence is the state.
pub const MARK: &str = "keychain-waits";

/// The reads of the first 10 minutes come every 30 s, so that a person who allows the daemon
/// sees it push soon; then every 5 minutes, so that a computer with nobody at it is not busy.
const QUICK_TRIES: u32 = 20;
const QUICK: Duration = Duration::from_secs(30);
const SLOW: Duration = Duration::from_mins(5);

/// Why a bounded read gave no answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Unread {
    /// The read did not end by its deadline: macOS waits for the person.
    #[error("the Keychain waits for the person to allow this program")]
    Waits,
    /// The thread of the read ended with no answer, or could not start.
    #[error("the Keychain read ended with no answer")]
    Lost,
}

/// The reads of one Keychain item: at most one OS call in flight.
pub struct Reads<T> {
    in_flight: Mutex<Option<Receiver<T>>>,
}

impl<T: Send + 'static> Reads<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            in_flight: Mutex::new(None),
        }
    }

    /// The answer of `call`, run on a thread of its own, or of the read that is still in
    /// flight (then `call` is not run). Waits `deadline` at most.
    ///
    /// # Errors
    /// [`Unread::Waits`] when no answer came by `deadline`; the read stays in flight.
    /// [`Unread::Lost`] when its thread ended with no answer.
    pub fn read(
        &self,
        deadline: Duration,
        call: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, Unread> {
        let mut in_flight = self
            .in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let answer = in_flight.take().unwrap_or_else(|| start(call));
        match answer.recv_timeout(deadline) {
            Ok(value) => Ok(value),
            Err(RecvTimeoutError::Timeout) => {
                *in_flight = Some(answer);
                Err(Unread::Waits)
            }
            Err(RecvTimeoutError::Disconnected) => Err(Unread::Lost),
        }
    }
}

impl<T: Send + 'static> Default for Reads<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs `call` on a new thread and gives the receiver of its answer. A thread that cannot
/// start drops the sender, and the receiver then reads [`Unread::Lost`].
fn start<T: Send + 'static>(call: impl FnOnce() -> T + Send + 'static) -> Receiver<T> {
    let (sender, answer) = mpsc::sync_channel(1);
    let started = thread::Builder::new()
        .name("keychain-read".to_owned())
        .spawn(move || {
            // The send fails only when no caller keeps the receiver; the answer is then unused.
            let _ = sender.send(call());
        });
    drop(started);
    answer
}

/// The pause before the next read, after `waited` reads that waited.
#[must_use]
pub const fn next_try(waited: u32) -> Duration {
    if waited < QUICK_TRIES { QUICK } else { SLOW }
}

/// Puts [`MARK`] in the daemon's folder when `waits`, and takes it away when not. A mark that
/// is already in that state is not an error.
///
/// # Errors
/// The I/O error of the folder or the file.
pub fn mark(home: &Home, waits: bool) -> io::Result<()> {
    let path = home.dir().join(MARK);
    if waits {
        home.ensure().map_err(io::Error::other)?;
        return std::fs::write(path, b"");
    }
    match std::fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        removed => removed,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, mpsc};

    use super::*;

    const SHORT: Duration = Duration::from_millis(50);
    const LONG: Duration = Duration::from_secs(10);

    #[test]
    fn met_013_a_read_that_ends_in_time_gives_its_answer() {
        let reads = Reads::new();
        assert_eq!(reads.read(LONG, || 7), Ok(7));
    }

    #[test]
    fn met_013_a_blocked_read_waits_and_no_second_read_starts_until_it_ends() {
        let reads = Reads::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let (click, clicked) = mpsc::channel::<()>();
        let counted = Arc::clone(&calls);
        let first = reads.read(SHORT, move || {
            counted.fetch_add(1, Ordering::SeqCst);
            clicked.recv().ok();
            "allowed"
        });
        assert_eq!(first, Err(Unread::Waits));
        for _ in 0..3 {
            let counted = Arc::clone(&calls);
            let again = reads.read(SHORT, move || {
                counted.fetch_add(1, Ordering::SeqCst);
                "a second read"
            });
            assert_eq!(again, Err(Unread::Waits));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1, "one read in flight");

        click.send(()).expect("the read waits");
        assert_eq!(reads.read(LONG, || "a new read"), Ok("allowed"));
        assert_eq!(
            reads.read(LONG, || "a new read"),
            Ok("a new read"),
            "after the answer, the next read starts afresh"
        );
    }

    #[test]
    fn met_013_a_read_whose_thread_panics_is_lost_and_the_next_starts_afresh() {
        let reads = Reads::new();
        let lost = reads.read(LONG, || -> u8 { panic!("the read") });
        assert_eq!(lost, Err(Unread::Lost));
        assert_eq!(reads.read(LONG, || 1), Ok(1));
    }

    #[test]
    fn met_013_reads_come_every_30_s_for_10_minutes_then_every_5_minutes() {
        assert_eq!(next_try(0), Duration::from_secs(30));
        assert_eq!(next_try(19), Duration::from_secs(30));
        assert_eq!(next_try(20), Duration::from_mins(5));
        assert_eq!(next_try(u32::MAX), Duration::from_mins(5));
    }

    #[test]
    fn met_013_the_mark_is_put_and_taken_away_twice_with_no_error() {
        let dir = tempfile::tempdir().expect("a folder");
        let home = Home::at(dir.path().join("daemon"));
        let path = home.dir().join(MARK);
        mark(&home, false).expect("no mark to take away");
        mark(&home, true).expect("put");
        mark(&home, true).expect("put again");
        assert!(path.is_file());
        mark(&home, false).expect("taken away");
        mark(&home, false).expect("taken away again");
        assert!(!path.exists());
    }
}

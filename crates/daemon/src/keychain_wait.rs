//! Keychain calls bounded by [`DEADLINE`]: macOS can wait for a click that never comes (R-322).

use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use zeroize::Zeroizing;

use crate::config::home::Home;

mod tries;

pub use tries::{Tries, next_try};

/// Well above the first unlock of the login keychain, below one 30 s step and the app's deadline (R-322).
pub const DEADLINE: Duration = Duration::from_secs(5);

/// Present in the daemon's folder while the daemon waits for the person in the Keychain; the app reads it.
pub const MARK: &str = "keychain-waits";

/// The least time a call gets after the call in flight answered late, so a waiting call is not given up at once.
const LEAST: Duration = Duration::from_secs(1);

/// The error is text so that it crosses the thread.
pub type Read = Result<Option<Zeroizing<Vec<u8>>>, String>;

pub type Written = Result<(), String>;

enum Answer {
    Read(Read),
    Written(Written),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Unanswered {
    #[error("the Keychain waits for the person to allow this program")]
    Waits,
    #[error("the Keychain call ended with no answer")]
    Lost,
}

/// The calls of one Keychain item, reads, writes and deletes: at most one OS call in flight.
pub struct Calls {
    in_flight: Mutex<Option<Receiver<Answer>>>,
}

impl Calls {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            in_flight: Mutex::new(None),
        }
    }

    /// The answer of `call`, or of a read still in flight, which then serves this one.
    ///
    /// # Errors
    /// [`Unanswered::Waits`] when no answer came by `deadline`; [`Unanswered::Lost`] when none came.
    pub fn read(
        &self,
        deadline: Duration,
        call: impl FnOnce() -> Read + Send + 'static,
    ) -> Result<Read, Unanswered> {
        let read = |answer| match answer {
            Answer::Read(read) => Some(read),
            Answer::Written(_) => None,
        };
        self.run::<_, true>(deadline, move || Answer::Read(call()), read)
    }

    /// Runs `call` once the call in flight ended; `deadline` bounds the two together.
    ///
    /// # Errors
    /// [`Unanswered::Waits`] when no answer came by `deadline`; [`Unanswered::Lost`] when none came.
    pub fn write(
        &self,
        deadline: Duration,
        call: impl FnOnce() -> Written + Send + 'static,
    ) -> Result<Written, Unanswered> {
        let written = |answer| match answer {
            Answer::Written(written) => Some(written),
            Answer::Read(_) => None,
        };
        self.run::<_, false>(deadline, move || Answer::Written(call()), written)
    }

    fn run<R, const LATE: bool>(
        &self,
        deadline: Duration,
        call: impl FnOnce() -> Answer + Send + 'static,
        kind: fn(Answer) -> Option<R>,
    ) -> Result<R, Unanswered> {
        let end = Instant::now() + deadline;
        let mut in_flight = self
            .in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(earlier) = in_flight.take() {
            match earlier.recv_timeout(deadline) {
                Err(RecvTimeoutError::Timeout) => {
                    *in_flight = Some(earlier);
                    return Err(Unanswered::Waits);
                }
                Ok(answer) => {
                    if LATE && let Some(value) = kind(answer) {
                        return Ok(value);
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {}
            }
        }
        let answer = start(call);
        let left = end
            .saturating_duration_since(Instant::now())
            .max(deadline.min(LEAST));
        match answer.recv_timeout(left) {
            Ok(answer) => kind(answer).ok_or(Unanswered::Lost),
            Err(RecvTimeoutError::Timeout) => {
                *in_flight = Some(answer);
                Err(Unanswered::Waits)
            }
            Err(RecvTimeoutError::Disconnected) => Err(Unanswered::Lost),
        }
    }
}

impl Default for Calls {
    fn default() -> Self {
        Self::new()
    }
}

/// A thread that cannot start drops the sender, so the receiver reads [`Unanswered::Lost`].
fn start<T: Send + 'static>(call: impl FnOnce() -> T + Send + 'static) -> Receiver<T> {
    let (sender, answer) = mpsc::sync_channel(1);
    let started = thread::Builder::new()
        .name("keychain-call".to_owned())
        .spawn(move || {
            // The send fails only when no caller keeps the receiver; the answer is then unused.
            let _ = sender.send(call());
        });
    drop(started);
    answer
}

/// Runs a store call that can take its whole deadline off the async loop; a panic of `call` goes on in the caller.
pub async fn off_loop<T: Send + 'static>(call: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(call)
        .await
        .unwrap_or_else(|ended| std::panic::resume_unwind(ended.into_panic()))
}

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

    #[expect(
        clippy::unnecessary_wraps,
        reason = "the answer of a read stand-in, which a read call returns as it is"
    )]
    fn bytes(text: &str) -> Read {
        Ok(Some(Zeroizing::new(text.as_bytes().to_vec())))
    }

    fn text(read: Result<Read, Unanswered>) -> Result<String, Unanswered> {
        read.map(|read| {
            let bytes = read.expect("an answer").expect("an item");
            String::from_utf8(bytes.to_vec()).expect("text")
        })
    }

    fn blocked<T: Send + 'static>(
        calls: &Arc<AtomicUsize>,
        answer: T,
    ) -> (
        mpsc::Sender<()>,
        impl FnOnce() -> T + Send + 'static + use<T>,
    ) {
        let (click, clicked) = mpsc::channel::<()>();
        let counted = Arc::clone(calls);
        let call = move || {
            counted.fetch_add(1, Ordering::SeqCst);
            clicked.recv().ok();
            answer
        };
        (click, call)
    }

    #[test]
    fn met_013_a_read_that_ends_in_time_gives_its_answer() {
        let calls = Calls::new();
        assert_eq!(text(calls.read(LONG, || bytes("7"))), Ok("7".to_owned()));
    }

    #[test]
    fn met_013_a_blocked_read_waits_and_no_second_read_starts_until_it_ends() {
        let calls = Calls::new();
        let count = Arc::new(AtomicUsize::new(0));
        let (click, call) = blocked(&count, bytes("allowed"));
        assert_eq!(text(calls.read(SHORT, call)), Err(Unanswered::Waits));
        for _ in 0..3 {
            let counted = Arc::clone(&count);
            let again = calls.read(SHORT, move || {
                counted.fetch_add(1, Ordering::SeqCst);
                bytes("a second read")
            });
            assert_eq!(text(again), Err(Unanswered::Waits));
        }
        assert_eq!(count.load(Ordering::SeqCst), 1, "one read in flight");

        click.send(()).expect("the read waits");
        let after = || calls.read(LONG, || bytes("a new read"));
        assert_eq!(text(after()), Ok("allowed".to_owned()));
        assert_eq!(
            text(after()),
            Ok("a new read".to_owned()),
            "after the answer, the next read starts afresh"
        );
    }

    #[test]
    fn met_013_a_read_whose_thread_panics_is_lost_and_the_next_starts_afresh() {
        let calls = Calls::new();
        let lost = calls.read(LONG, || -> Read { panic!("the read") });
        assert_eq!(lost.err(), Some(Unanswered::Lost));
        assert_eq!(text(calls.read(LONG, || bytes("1"))), Ok("1".to_owned()));
    }

    #[test]
    fn met_013_a_blocked_write_waits_within_its_deadline_and_no_second_write_starts() {
        let calls = Calls::new();
        let count = Arc::new(AtomicUsize::new(0));
        let (click, call) = blocked(&count, Ok(()));
        let began = Instant::now();
        assert_eq!(calls.write(SHORT, call), Err(Unanswered::Waits));
        assert!(
            began.elapsed() < Duration::from_secs(1),
            "it gave up at its deadline of 50 ms"
        );
        let counted = Arc::clone(&count);
        let second = calls.write(SHORT, move || {
            counted.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        assert_eq!(second, Err(Unanswered::Waits));
        assert_eq!(count.load(Ordering::SeqCst), 1, "one write in flight");

        click.send(()).expect("the write waits");
        let counted = Arc::clone(&count);
        let third = calls.write(LONG, move || {
            counted.fetch_add(1, Ordering::SeqCst);
            Err("newer bytes".to_owned())
        });
        assert_eq!(
            third,
            Ok(Err("newer bytes".to_owned())),
            "a write runs after the one in flight ended, as its bytes may be newer"
        );
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn met_013_a_read_and_a_write_of_one_item_never_run_at_once() {
        let calls = Calls::new();
        let count = Arc::new(AtomicUsize::new(0));
        let (click, call) = blocked(&count, bytes("old"));
        assert_eq!(text(calls.read(SHORT, call)), Err(Unanswered::Waits));
        let counted = Arc::clone(&count);
        let write = calls.write(SHORT, move || {
            counted.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        assert_eq!(write, Err(Unanswered::Waits), "the read is still in flight");
        assert_eq!(count.load(Ordering::SeqCst), 1);

        click.send(()).expect("the read waits");
        assert_eq!(
            calls.write(LONG, || Ok(())),
            Ok(Ok(())),
            "not the read's answer"
        );
        let (click, call) = blocked(&count, Ok(()));
        assert_eq!(calls.write(SHORT, call), Err(Unanswered::Waits));
        drop(click);
        assert_eq!(
            text(calls.read(LONG, || bytes("after the write"))),
            Ok("after the write".to_owned()),
            "a read after a write reads the item, not the write's answer"
        );
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

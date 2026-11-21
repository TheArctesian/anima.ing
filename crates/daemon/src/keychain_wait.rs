//! A Keychain call that cannot hold the daemon for ever (R-322). macOS asks the person before
//! a program that is not on the access list of an item reads, writes or deletes it; an ad hoc
//! signed build is a new program at each build, so its call can wait for a click that never
//! comes, and the OS call has no timeout.
//!
//! The call runs on a thread of its own and the caller waits [`DEADLINE`] at most. The blocked
//! OS call cannot be cancelled, so a call that passed its deadline is left to end; the next
//! call on the same item waits for that one and starts no second thread. So one OS call is in
//! flight for each item, reads and writes together. The answer that the person gives late to
//! a read is the answer of the next read; a write always runs after the call in flight ends,
//! as its bytes may be newer. While the daemon waits, the file [`MARK`] in its folder tells
//! the app (which cannot ask the hub for it: without the token the daemon cannot reach the
//! hub).

use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use zeroize::Zeroizing;

use crate::config::home::Home;

/// How long a caller waits for one Keychain call. An allowed call ends in milliseconds, and
/// the first unlock of the login keychain in a second or two; a prompt that nobody answers
/// never ends. 5 s is well above the first and keeps `install` far below the 30 s of one
/// step and the deadline of the app (R-322).
pub const DEADLINE: Duration = Duration::from_secs(5);

/// The file in the daemon's folder that is there while the daemon waits for the person to
/// allow it in the Keychain. Empty: its presence is the state.
pub const MARK: &str = "keychain-waits";

/// The tries of the first 10 minutes come every 30 s, so that a person who allows the daemon
/// sees it push soon; then every 5 minutes, so that a computer with nobody at it is not busy.
const QUICK_TRIES: u32 = 20;
const QUICK: Duration = Duration::from_secs(30);
const SLOW: Duration = Duration::from_mins(5);

/// What one read of an item gives: its bytes, `None` when there is no item, or the error as
/// text, so that it crosses the thread.
pub type Read = Result<Option<Zeroizing<Vec<u8>>>, String>;

/// What one write or delete of an item gives: the error as text.
pub type Written = Result<(), String>;

/// The answer of the OS call in flight: a read, or a write or delete.
enum Answer {
    Read(Read),
    Written(Written),
}

/// Why a bounded call gave no answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Unanswered {
    /// The call did not end by its deadline: macOS waits for the person.
    #[error("the Keychain waits for the person to allow this program")]
    Waits,
    /// The thread of the call ended with no answer, or could not start.
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

    /// The answer of `call`, run on a thread of its own, or of the read that is still in
    /// flight (then `call` is not run). After a write in flight, `call` runs once it ended.
    /// Waits `deadline` at most.
    ///
    /// # Errors
    /// [`Unanswered::Waits`] when no answer came by `deadline`; the OS call stays in flight.
    /// [`Unanswered::Lost`] when its thread ended with no answer.
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

    /// The answer of `call`, a write or a delete, run on a thread of its own once the call in
    /// flight, if any, ended. Waits `deadline` at most, for the two together.
    ///
    /// # Errors
    /// [`Unanswered::Waits`] when no answer came by `deadline`: the call in flight is still
    /// blocked and `call` is never run, or `call` is blocked and stays in flight.
    /// [`Unanswered::Lost`] when its thread ended with no answer.
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

    /// Waits for the call in flight; gives its answer when `LATE` and `kind` takes it, else
    /// runs `call` in the time that is left of `deadline`.
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
        match answer.recv_timeout(end.saturating_duration_since(Instant::now())) {
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

/// Runs `call` on a new thread and gives the receiver of its answer. A thread that cannot
/// start drops the sender, and the receiver then reads [`Unanswered::Lost`].
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

/// The pause before the next try, after `waited` tries that waited.
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

    /// A call that counts itself in `calls`, then blocks until `click` is sent or dropped.
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
        assert!(began.elapsed() < LONG, "it gave up at its deadline");
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
    fn met_013_tries_come_every_30_s_for_10_minutes_then_every_5_minutes() {
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

//! The secrets of the live threads (Section 7.3): 32 random bytes for each thread, in memory
//! only, dead when the thread ends.

use std::fmt::Write as _;
use std::sync::Mutex;

use subtle::ConstantTimeEq as _;

use crate::upstream::Program;

/// The characters of a secret: 32 bytes in hexadecimal.
pub const SECRET_LENGTH: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("The relay is open for this thread already.")]
    AlreadyOpen,
    #[error("The system gave no random bytes for the secret of the thread.")]
    NoRandomBytes,
    #[error("The name of a thread is 1 to 128 characters: letters, digits and . _ : -")]
    NotAName,
}

/// A thread that the relay serves, with its secret. It has no `Debug` and no `Clone`: the
/// secret stays in the list.
struct Open {
    thread: String,
    program: Program,
    secret: String,
}

/// Whether `thread` can be the name of a thread. A log line holds the name, so it has no
/// character that could start a new line or look like a field.
fn is_name(thread: &str) -> bool {
    let fits = |each: char| each.is_ascii_alphanumeric() || matches!(each, '.' | '_' | ':' | '-');
    (1..=128).contains(&thread.len()) && thread.chars().all(fits)
}

#[derive(Default)]
pub struct Secrets {
    open: Mutex<Vec<Open>>,
}

impl Secrets {
    fn locked(&self) -> std::sync::MutexGuard<'_, Vec<Open>> {
        // A panic while the list was held leaves a list that is still whole.
        self.open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn open(&self, thread: &str, program: Program) -> Result<String, SecretError> {
        if !is_name(thread) {
            return Err(SecretError::NotAName);
        }
        let mut open = self.locked();
        if open.iter().any(|each| each.thread == thread) {
            return Err(SecretError::AlreadyOpen);
        }
        let mut bytes = [0_u8; SECRET_LENGTH / 2];
        getrandom::fill(&mut bytes).map_err(|_| SecretError::NoRandomBytes)?;
        let mut secret = String::with_capacity(SECRET_LENGTH);
        for byte in bytes {
            // Writing to a `String` cannot fail.
            let _ = write!(secret, "{byte:02x}");
        }
        open.push(Open {
            thread: thread.to_owned(),
            program,
            secret: secret.clone(),
        });
        Ok(secret)
    }

    pub fn close(&self, thread: &str) {
        self.locked().retain(|each| each.thread != thread);
    }

    /// Ends each secret.
    pub fn close_all(&self) {
        self.locked().clear();
    }

    /// The name and the program of the thread whose secret is `given`. Each live secret is
    /// compared, each in constant time, so the time says nothing of how much of a secret was
    /// right.
    pub fn thread_of(&self, given: &str) -> Option<(String, Program)> {
        let mut found = None;
        for each in self.locked().iter() {
            if bool::from(each.secret.as_bytes().ct_eq(given.as_bytes())) {
                found = Some((each.thread.clone(), each.program));
            }
        }
        found
    }
}

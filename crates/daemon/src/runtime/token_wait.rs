//! The token at the start of `run`, when the Keychain makes the daemon wait for the person
//! (R-NEW-keychainwait): the daemon does not end, so the service manager does not start it
//! again and again. It logs one line, puts the mark for the app, and reads again on the
//! schedule of [`keychain_wait::next_try`] until the read answers. Each read runs off the async
//! loop; the runner of the agent threads runs on beside it.

use std::time::Duration;

use crate::config::home::Home;
use crate::keychain_wait;
use crate::report;
use crate::token_store::{Credential, TokenStore, TokenStoreError};

/// The stored credential of the daemon in `home`, once the Keychain answers.
///
/// # Errors
/// [`TokenStoreError`] other than [`TokenStoreError::KeychainWaits`].
pub async fn credential(home: &Home) -> Result<Option<Credential>, TokenStoreError> {
    let read = || {
        let store = TokenStore::open(home);
        async move {
            tokio::task::spawn_blocking(move || store.load())
                .await
                .unwrap_or_else(|ended| std::panic::resume_unwind(ended.into_panic()))
        }
    };
    let mark = |waits| {
        if let Err(error) = keychain_wait::mark(home, waits) {
            report::keychain_mark_refused(&error);
        }
    };
    answered(read, tokio::time::sleep, mark).await
}

/// Reads with `read` until it gives anything but [`TokenStoreError::KeychainWaits`], with
/// `pause` between two reads. `mark` gets `true` at the first wait and `false` at the end.
async fn answered<T, R, P>(
    mut read: impl FnMut() -> R,
    mut pause: impl FnMut(Duration) -> P,
    mut mark: impl FnMut(bool),
) -> Result<T, TokenStoreError>
where
    R: Future<Output = Result<T, TokenStoreError>>,
    P: Future<Output = ()>,
{
    let mut waited = 0_u32;
    loop {
        match read().await {
            Err(TokenStoreError::KeychainWaits) => {
                if waited == 0 {
                    report::keychain_waits();
                    mark(true);
                }
                pause(keychain_wait::next_try(waited)).await;
                waited = waited.saturating_add(1);
            }
            ended => {
                if waited > 0 {
                    report::keychain_answered();
                }
                mark(false);
                return ended;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// Runs [`answered`] with reads that wait `waits` times, then give 7; returns the result,
    /// the pauses and the marks.
    async fn after(waits: usize) -> (Result<u8, TokenStoreError>, Vec<Duration>, Vec<bool>) {
        let reads = RefCell::new(0_usize);
        let pauses = RefCell::new(Vec::new());
        let marks = RefCell::new(Vec::new());
        let read = || {
            *reads.borrow_mut() += 1;
            let waiting = *reads.borrow() <= waits;
            async move {
                if waiting {
                    Err(TokenStoreError::KeychainWaits)
                } else {
                    Ok(7)
                }
            }
        };
        let pause = |wait| {
            pauses.borrow_mut().push(wait);
            std::future::ready(())
        };
        let result = answered(read, pause, |waits| marks.borrow_mut().push(waits)).await;
        (result, pauses.into_inner(), marks.into_inner())
    }

    #[tokio::test]
    async fn met_013_a_token_read_that_waits_is_read_again_until_the_keychain_answers() {
        let (result, pauses, marks) = after(22).await;
        assert_eq!(result.expect("answered"), 7);
        assert_eq!(pauses.len(), 22);
        assert!(pauses[..20].iter().all(|p| *p == Duration::from_secs(30)));
        assert!(pauses[20..].iter().all(|p| *p == Duration::from_mins(5)));
        assert_eq!(
            marks,
            [true, false],
            "marked once, taken away at the answer"
        );
    }

    #[tokio::test]
    async fn met_013_a_token_read_that_answers_at_once_does_not_pause() {
        let (result, pauses, marks) = after(0).await;
        assert_eq!(result.expect("answered"), 7);
        assert_eq!(pauses, Vec::<Duration>::new());
        assert_eq!(marks, [false], "a mark of an earlier wait is taken away");
    }
}

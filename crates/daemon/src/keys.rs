//! The operational keys of this machine and the local reader `anima-daemon key read` (K-E5
//! Section 2.1, R-147). No type that holds a value implements `Debug` or `Display`, and no log
//! line holds a value or a key name (LOG-007).

pub mod cache;
pub mod keeper;
mod pending;
pub mod record;
pub mod store;

use chrono::{DateTime, Utc};

use cache::KeyCache;
use store::{KeyStore, KeyStoreError};

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("standard output is a terminal; a key is written only to a pipe or a file")]
    Terminal,
    #[error("this machine holds no key of that name")]
    NotHeld,
    #[error("{0}")]
    Store(#[from] KeyStoreError),
    #[error("the key could not be written: {0}")]
    Write(std::io::ErrorKind),
}

/// # Errors
/// [`ReadError`] for a terminal, a key the cache does not hold, or a store or write failure.
pub fn read(
    store: &KeyStore,
    (name, now): (&str, DateTime<Utc>),
    (out, to_terminal): (&mut dyn std::io::Write, bool),
) -> Result<(), ReadError> {
    if to_terminal {
        return Err(ReadError::Terminal);
    }
    let cache: KeyCache = store.load()?;
    if cache.is_stale(now) {
        store.forget()?;
        return Err(ReadError::NotHeld);
    }
    let value = cache.value(name).ok_or(ReadError::NotHeld)?;
    out.write_all(value.expose())
        .and_then(|()| out.flush())
        .map_err(|error| ReadError::Write(error.kind()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::home::Home;

    #[test]
    fn r_147_key_read_refuses_a_terminal_before_it_opens_the_store() {
        let home = Home::at(std::path::PathBuf::from("/nonexistent/anima-daemon"));
        let mut out = Vec::new();
        let asked = ("cloudflare.dns", DateTime::UNIX_EPOCH);
        let refused = read(&KeyStore::open(&home), asked, (&mut out, true));
        assert!(matches!(refused, Err(ReadError::Terminal)), "{refused:?}");
        assert_eq!(out, Vec::<u8>::new());
    }
}

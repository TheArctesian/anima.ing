//! The tauri-updater signature of an Anima release (BLD-045, BLD-046, UPD-002, R-117).
//!
//! In-house minisign (WP-36 D-11): no `minisign` crate outside the tests.
//! Pure: no file, no clock, no log. The hub reads the key file and passes the bytes in.
//! The secret key has no `Debug`, `Display` or `Clone`, and its seed is wiped on drop (LOG-007).

mod error;
mod key_file;
mod sealed;
mod signature;

#[cfg(test)]
mod tests;

pub use error::SignatureError;
pub use key_file::{PublicKey, SecretKey};
pub use signature::{Prehash, SignatureCheck, SignatureStream, Signed};

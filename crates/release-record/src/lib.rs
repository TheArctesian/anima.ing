//! The signed list of the program files of one release for one target (ROL-003 to ROL-005, ROL-010 to ROL-013, ROL-036, ROL-041).
//! Its signature is the updater signature, one key (OD-ROL-2).
//!
//! The bytes that were signed are the bytes that are read: no form of the record is made again before its check (ROL-004).
//! Pure: no file, no clock, no network, no log.

mod check;
mod error;
mod model;
mod sign;
mod version;

#[cfg(test)]
mod tests;

pub use check::{Admitted, Checked, Expected, FileCheck, admits, check};
pub use error::Refusal;
pub use model::{Database, FORMAT, File, Program, Record, Target};
pub use sign::{SignError, sign};
pub use version::{Version, VersionError};

/// The name of the record on the release host and in its signature's trusted comment (ROL-005).
pub const RECORD_FILE: &str = "release-record.json";

pub const SIGNATURE_FILE: &str = "release-record.json.sig";

/// The largest record that is read (K8 Section 8.3).
pub const MAX_RECORD_BYTES: usize = 64 * 1024;

/// The largest program file that a record can name (K8 Section 8.3).
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

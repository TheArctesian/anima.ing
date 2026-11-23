//! Pure syntax check of Compose text and conversion of `docker run` (HOST-022, R-150).
//! A problem holds a fixed code, a position and at most a service and key name, never a value of
//! the file; the hub logs neither text (LOG-007).

mod docker_run;
mod draft;
mod keys;
mod mounts;
mod options;
mod syntax;
#[cfg(test)]
mod tests;
mod words;
mod writer;
mod yaml;

pub use self::docker_run::{Converted, Note, Refusal, RefusalCode, convert};
pub use self::syntax::{Check, Problem, ProblemCode, Severity, check};
pub use self::yaml::Place;

/// The most bytes of a Compose file that the check reads.
pub const LARGEST_FILE: usize = 256 * 1024;
/// The most bytes of a `docker run` command that the converter reads.
pub const LARGEST_COMMAND: usize = 64 * 1024;

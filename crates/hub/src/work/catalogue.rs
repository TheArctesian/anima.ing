//! The catalogue that ships with Anima (HOST-047, R-299, R-300, task H38).
//! No read and no install reaches a machine, keeps anything or writes a log line.

pub mod graphql;
mod install;
mod listing;
mod model;
mod parts;
#[cfg(test)]
mod tests;

use std::sync::LazyLock;

pub use self::install::{Ask, Install, Refusal, install};
pub use self::model::{EntryKind, Listed, MarkKind};

/// The entries that passed the static test, in the order of their identifiers.
static LISTED: LazyLock<Vec<Listed>> = LazyLock::new(|| listing::listed(::catalogue::shipped()));

#[must_use]
pub fn entries() -> &'static [Listed] {
    &LISTED
}

#[must_use]
pub fn entry(id: &str) -> Option<&'static Listed> {
    entries().iter().find(|listed| listed.entry.id == id)
}

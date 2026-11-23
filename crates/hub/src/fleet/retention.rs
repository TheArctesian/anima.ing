//! How long each kind of data is kept (HUB-005, DB-011, D-25).

mod bounds;
pub mod graphql;
mod model;
mod store;
#[cfg(test)]
mod tests;

use chrono::Days;
use sqlx::PgPool;

use crate::api_errors::{FieldError, field_names};

pub use model::Subject;

/// Why a retention operation failed. Messages hold no SQL and no input value.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("input is not valid: {}", field_names(.0))]
    Validation(Vec<FieldError>),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    /// A stored period is missing or out of bounds; no period is guessed.
    #[error("the stored retention periods cannot be read")]
    Unreadable,
}

impl Error {
    /// For the process log: a fixed word, never the source text (LOG-007, API-010).
    #[must_use]
    pub const fn fault(&self) -> &'static str {
        match self {
            Self::Database(_) => "database",
            Self::Validation(_) => "validation",
            Self::Unreadable => "unreadable",
        }
    }
}

/// The stored period of every subject, in days.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Periods([u16; Subject::ALL.len()]);

impl Periods {
    #[must_use]
    pub fn keep(&self, subject: Subject) -> Days {
        Days::new(u64::from(self.days(subject)))
    }

    const fn days(self, subject: Subject) -> u16 {
        self.0[subject as usize]
    }

    fn from_rows(rows: &[(String, i32)]) -> Result<Self, Error> {
        let mut days = [None; Subject::ALL.len()];
        for (name, stored) in rows {
            let subject = Subject::from_stored(name).ok_or(Error::Unreadable)?;
            let checked =
                bounds::checked(subject, i64::from(*stored)).map_err(|_| Error::Unreadable)?;
            days[subject as usize] = Some(checked);
        }
        let all: Option<Vec<u16>> = days.into_iter().collect();
        all.and_then(|all| all.try_into().ok())
            .map(Self)
            .ok_or(Error::Unreadable)
    }
}

/// # Errors
/// `Database` when the read fails; `Unreadable` when a subject has no row or a value is out of bounds.
pub async fn periods(db: &PgPool) -> Result<Periods, Error> {
    let rows = store::all(db).await?;
    Periods::from_rows(&rows)
}

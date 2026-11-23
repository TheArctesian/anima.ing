//! Record links: a record of one extension points at one of another, so no extension imports
//! another or waits for it (RLK-001, RLK-002, RLK-004).

pub mod graphql;
mod model;
mod store;
mod titles;
mod trail;

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};

pub use model::{Link, NewLink, Outcome, Relation, label};
pub use titles::{TitleSources, Titles, Unavailable};

use uuid::Uuid;

use crate::{
    engine::records::{Reference, Subject},
    ids::RecordLinkId,
};

/// The most links [`touching`] returns for one record.
pub const MAX_LINKS: usize = 200;

/// Messages hold no SQL and no input value.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("a record cannot link to itself")]
    SelfLink,
    #[error("{0} is not a valid subject key")]
    BadSubject(&'static str),
}

fn checked(reference: &Reference, side: &'static str) -> Result<(), Error> {
    if Subject::owns(reference) && Subject::of(reference).is_none() {
        Err(Error::BadSubject(side))
    } else {
        Ok(())
    }
}

/// A second call with the same link writes nothing, and one with a new label writes the label.
/// Neither record has to exist (RLK-002).
///
/// # Errors
/// `SelfLink`, `BadSubject`; `Database` when a statement fails. The caller rolls back.
pub async fn connect(
    conn: &mut PgConnection,
    new: NewLink,
    (at, by): (DateTime<Utc>, &str),
) -> Result<(Link, Outcome), Error> {
    if new.source == new.target {
        return Err(Error::SelfLink);
    }
    checked(&new.source, "source")?;
    checked(&new.target, "target")?;
    if let Some(made) = store::insert(conn, &new, (at, by)).await? {
        return Ok((made, Outcome::Made));
    }
    let Some(stored) = store::existing(conn, &new).await? else {
        // Deleted between the two statements: make it now.
        let made = store::insert(conn, &new, (at, by)).await?;
        return made
            .map(|link| (link, Outcome::Made))
            .ok_or(Error::Database(sqlx::Error::RowNotFound));
    };
    match new.label {
        Some(label) if stored.label.as_deref() != Some(&label) => {
            store::set_label(conn, stored.id, &label).await?;
            let relabelled = Link {
                label: Some(label),
                ..stored
            };
            Ok((relabelled, Outcome::Relabelled))
        }
        _ => Ok((stored, Outcome::Kept)),
    }
}

/// The two records are not touched.
///
/// # Errors
/// `Database` when the statement fails.
pub async fn disconnect(conn: &mut PgConnection, id: RecordLinkId) -> Result<Option<Link>, Error> {
    Ok(store::delete(conn, id).await?)
}

/// Newest first, at most [`MAX_LINKS`].
///
/// # Errors
/// `Database` when the read fails.
pub async fn touching(db: &PgPool, record: &Reference) -> Result<Vec<Link>, Error> {
    let limit = i64::try_from(MAX_LINKS).unwrap_or(i64::MAX);
    Ok(store::touching(db, record, limit).await?)
}

/// A link that would join a record to itself, or repeat a link of `to`, is removed (PLC-004).
///
/// # Errors
/// `SelfLink`, `BadSubject`; `Database` when a statement fails. The caller rolls back.
pub async fn retarget(
    conn: &mut PgConnection,
    from: &Reference,
    to: &Reference,
) -> Result<u64, Error> {
    if from == to {
        return Err(Error::SelfLink);
    }
    checked(to, "target")?;
    Ok(store::retarget(conn, (from, to), None).await?)
}

/// A name that is not a link of `from` moves nothing, so the caller compares the count (PLC-004).
///
/// # Errors
/// As [`retarget`].
pub async fn retarget_named(
    conn: &mut PgConnection,
    ids: &[RecordLinkId],
    (from, to): (&Reference, &Reference),
) -> Result<u64, Error> {
    if from == to {
        return Err(Error::SelfLink);
    }
    checked(to, "target")?;
    let named: Vec<Uuid> = ids.iter().map(|id| Uuid::from(*id)).collect();
    Ok(store::retarget(conn, (from, to), Some(&named)).await?)
}

/// All of them, not only the newest [`MAX_LINKS`].
///
/// # Errors
/// `Database` when the read fails.
pub async fn count(conn: &mut PgConnection, record: &Reference) -> Result<u64, Error> {
    let total = store::count(conn, record).await?;
    Ok(u64::try_from(total).unwrap_or_default())
}

/// A date or a repository is its own title; any other record gets its title from its domain
/// (RLK-002).
#[must_use]
pub fn own_title(record: &Reference) -> Option<String> {
    match Subject::of(record)? {
        Subject::Repository(name) => Some(name),
        Subject::Date(date) => Some(date.to_string()),
        Subject::Place(_) | Subject::Person(_) => None,
    }
}

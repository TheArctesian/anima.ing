//! What an extension hands the engine so it can feed it records (K-E1 Section 3). The engine
//! imports no extension (EXT-005): wiring builds a [`Registry`] and the engine asks it.

use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use source_exchange::{Declaration, Problem, Request, Response};
use sqlx::PgConnection;

use crate::{engine::extensions::Key, ids::SourceAccountId};

/// Obligations (SRC-011, SRC-012): keep the provider's original record beside the canonical
/// fields, and compare before writing, so an unchanged rerun or a replayed page reports only
/// `unchanged`.
#[async_trait]
pub trait Intake: Send + Sync {
    /// Its `Declaration::key`.
    fn source(&self) -> &'static str;
    /// The consumer: the engine uses this intake only while this extension is on.
    fn extension(&self) -> Key;
    /// Its key in the account's sync state (SRC-004).
    fn slice_key(&self) -> &'static str;
    /// The slice at the start of a run, from the stored one (`{}` when there is none).
    fn begin(&self, slice: &Value) -> Value;
    /// What to fetch next; `None` ends this intake's part of the run.
    fn next_request(&self, slice: &Value) -> Option<Request>;
    /// Normalises one response and stores its records in `conn`, the engine's transaction.
    ///
    /// # Errors
    /// [`IntakeError`] when the records cannot be stored; the engine rolls the page back.
    async fn take(&self, conn: &mut PgConnection, page: Page<'_>) -> Result<Taken, IntakeError>;
    /// # Errors
    /// [`IntakeError`] when they cannot be deleted; the engine keeps the account.
    async fn forget(
        &self,
        conn: &mut PgConnection,
        account: SourceAccountId,
    ) -> Result<u64, IntakeError>;
}

pub struct Page<'a> {
    pub account: SourceAccountId,
    pub request: &'a Request,
    pub response: Response<'a>,
    /// The intake's slice before this page.
    pub slice: &'a Value,
    pub now: DateTime<Utc>,
}

pub struct Taken {
    pub slice: Value,
    pub counts: Counts,
    pub problems: Vec<Problem>,
}

/// What a page or a run did to the stored records (SRC-005).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub inserted: u32,
    pub changed: u32,
    pub unchanged: u32,
    pub removed: u32,
}

/// Fixed words, never a record (LOG-007).
#[derive(Debug, thiserror::Error)]
pub enum IntakeError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("the records were refused: {0}")]
    Refused(&'static str),
}

#[derive(Clone)]
pub struct Registry {
    declarations: Arc<[Declaration]>,
    intakes: Arc<[Arc<dyn Intake>]>,
}

impl Registry {
    #[must_use]
    pub fn new(declarations: Vec<Declaration>, intakes: Vec<Arc<dyn Intake>>) -> Self {
        Self {
            declarations: declarations.into(),
            intakes: intakes.into(),
        }
    }

    #[must_use]
    pub fn declarations(&self) -> &[Declaration] {
        &self.declarations
    }

    #[must_use]
    pub fn declaration(&self, source: &str) -> Option<&Declaration> {
        self.declarations.iter().find(|d| d.key == source)
    }

    /// In wiring order.
    pub fn intakes_of<'a>(&'a self, source: &'a str) -> impl Iterator<Item = &'a Arc<dyn Intake>> {
        self.intakes.iter().filter(move |i| i.source() == source)
    }

    /// Each once, in wiring order.
    #[must_use]
    pub fn consumers(&self, source: &str) -> Vec<Key> {
        let mut keys: Vec<Key> = Vec::new();
        for key in self.intakes_of(source).map(|i| i.extension()) {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        keys
    }

    /// Whether an extension that uses `source` is in `on` (EXT-002).
    #[must_use]
    pub fn consumer_on(&self, source: &str, on: &HashSet<Key>) -> bool {
        self.intakes_of(source).any(|i| on.contains(&i.extension()))
    }
}

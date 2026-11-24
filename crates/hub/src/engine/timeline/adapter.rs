//! The engine imports no extension (EXT-005): wiring builds the [`Adapters`] (K-E3; TML-001).

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;

use super::model::{Ask, Evidence};

/// The message names the adapter only; the cause goes to the process log (API-010).
#[derive(Debug, thiserror::Error)]
#[error("the timeline adapter {adapter} could not read its records")]
pub struct Unavailable {
    pub adapter: &'static str,
    #[source]
    pub cause: Box<dyn std::error::Error + Send + Sync>,
}

/// Obligations: copy nothing (TML-001); return only entries `ask` wants, at most `ask.limit`,
/// newest first; keep an entry that a second source also recorded (TML-005); note each value the
/// row does not give (TML-006).
#[async_trait]
pub trait Adapter: Send + Sync {
    fn name(&self) -> &'static str;

    /// # Errors
    /// [`Unavailable`] when the records cannot be read.
    async fn evidence(&self, db: &PgPool, ask: &Ask) -> Result<Vec<Evidence>, Unavailable>;
}

#[derive(Clone, Default)]
pub struct Adapters(Arc<[Arc<dyn Adapter>]>);

impl Adapters {
    #[must_use]
    pub fn new(adapters: Vec<Arc<dyn Adapter>>) -> Self {
        Self(adapters.into())
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &Arc<dyn Adapter>> {
        self.0.iter()
    }
}

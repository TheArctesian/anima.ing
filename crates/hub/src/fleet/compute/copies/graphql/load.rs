//! The output type of a copy in memory (K1 `ModelLoad`; K11 Section 11.1).

use async_graphql::{ID, Object};
use chrono::{DateTime, Utc};

use super::super::loads::{Load, LoadState};
use super::types::float;

/// A copy that a server of its machine holds, as the last model report of the machine gave it.
#[derive(Clone, Debug)]
pub struct ModelLoad(pub Load);

#[Object]
impl ModelLoad {
    /// The model server that holds the copy.
    async fn server(&self) -> ID {
        ID(self.0.server.to_string())
    }

    async fn state(&self) -> LoadState {
        self.0.state
    }

    /// The memory that the machine reserved for the copy.
    async fn reserved_bytes(&self) -> f64 {
        float(self.0.reserved_bytes)
    }

    /// The machine unloads the copy after this many seconds with no use, unless it is pinned (FIT-022).
    async fn idle_seconds(&self) -> u32 {
        self.0.idle_seconds
    }

    /// A pinned copy stays loaded, and comes back when other work took its memory (FIT-023).
    async fn pinned(&self) -> bool {
        self.0.pinned
    }

    /// The seconds since its last use, rounded down to a minute; 0 while it is ACTIVE or LOADING.
    async fn used_seconds_ago(&self) -> f64 {
        float(self.0.used_seconds_ago)
    }

    /// When the machine made the report.
    async fn read_at(&self) -> DateTime<Utc> {
        self.0.read_at
    }
}

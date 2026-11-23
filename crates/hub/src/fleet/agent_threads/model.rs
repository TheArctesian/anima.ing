use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::ids::DeviceId;

#[derive(sqlx::FromRow)]
pub(super) struct Summary {
    pub id: Uuid,
    pub device_id: DeviceId,
    pub program: String,
    pub state: String,
    pub title: String,
    pub started_at: DateTime<Utc>,
    pub activity_at: DateTime<Utc>,
    pub repository: Option<String>,
}

#[derive(sqlx::FromRow)]
pub(super) struct Entry {
    pub sequence: i64,
    pub content: serde_json::Value,
}

#[derive(Default)]
pub(super) struct Filter {
    pub device: Option<DeviceId>,
    pub program: Option<String>,
    pub project: Option<String>,
    pub state: Option<String>,
    pub after: Option<(DateTime<Utc>, Uuid)>,
}

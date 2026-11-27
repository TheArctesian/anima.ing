#![expect(
    clippy::unused_async_trait_impl,
    reason = "the GraphQL enum derive generates async trait impls"
)]

use async_graphql::{Enum, ID, Result, SimpleObject};
use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use super::super::{Job, State, reasons, store, trail};
use crate::{api_errors::internal, ids::JobId, paging::PageEnd};

#[derive(Clone, Copy, PartialEq, Eq, Enum)]
pub(super) enum WorkJobState {
    Queued,
    Claimed,
    Preparing,
    Running,
    Finishing,
    Done,
    Failed,
    Stopped,
}

impl WorkJobState {
    pub(super) const fn word(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Claimed => "claimed",
            Self::Preparing => "preparing",
            Self::Running => "running",
            Self::Finishing => "finishing",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }
}

impl From<State> for WorkJobState {
    fn from(state: State) -> Self {
        match state {
            State::Queued => Self::Queued,
            State::Claimed => Self::Claimed,
            State::Preparing => Self::Preparing,
            State::Running => Self::Running,
            State::Finishing => Self::Finishing,
            State::Done => Self::Done,
            State::Failed => Self::Failed,
            State::Stopped => Self::Stopped,
        }
    }
}

#[derive(SimpleObject)]
struct WorkJob {
    id: ID,
    #[graphql(name = "type")]
    work_type: String,
    kind: String,
    project_id: Option<ID>,
    repository: Option<String>,
    state: WorkJobState,
    attempt: i32,
    device_id: Option<ID>,
    created_at: DateTime<Utc>,
    claimed_at: Option<DateTime<Utc>>,
    changed_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    /// Untrusted text. Machine names may come from the operator.
    placement: Option<String>,
}

#[derive(SimpleObject)]
pub(super) struct WorkJobPage {
    nodes: Vec<WorkJob>,
    page_info: PageEnd,
}

#[derive(SimpleObject)]
pub(super) struct WorkJobDetail {
    job: WorkJob,
    /// Untrusted text. Limited to 65536 UTF-8 bytes from the end of the redacted log.
    log: String,
}

pub(super) fn cursor(encoded: &str) -> Result<(DateTime<Utc>, JobId)> {
    let fault = || super::bad("after");
    let (at, id) = encoded.split_once('/').ok_or_else(fault)?;
    Ok((
        at.parse().map_err(|_| fault())?,
        id.parse().map_err(|_| fault())?,
    ))
}

pub(super) async fn page(
    conn: &mut PgConnection,
    mut rows: Vec<Job>,
    limit: usize,
) -> Result<WorkJobPage> {
    let has_next_page = rows.len() > limit;
    rows.truncate(limit);
    let end_cursor = rows
        .last()
        .map(|row| format!("{}/{}", row.created_at.to_rfc3339(), row.id));
    let mut nodes = Vec::with_capacity(rows.len());
    for row in rows {
        nodes.push(work(conn, row).await?);
    }
    Ok(WorkJobPage {
        nodes,
        page_info: PageEnd {
            has_next_page,
            end_cursor,
        },
    })
}

pub(super) async fn detail(conn: &mut PgConnection, id: JobId) -> Result<Option<WorkJobDetail>> {
    let Some(row) = store::job_locked(conn, id).await.map_err(internal)? else {
        return Ok(None);
    };
    let log = store::log_of(conn, id).await.map_err(internal)?;
    let mut log = trail::clean_log(&log);
    let start = log.ceil_char_boundary(log.len().saturating_sub(64 * 1024));
    log.drain(..start);
    Ok(Some(WorkJobDetail {
        job: work(conn, row).await?,
        log,
    }))
}

async fn work(conn: &mut PgConnection, row: Job) -> Result<WorkJob> {
    let decisions = store::decisions_of(conn, row.id).await.map_err(internal)?;
    let mut placement = reasons::line(&row, &decisions).replace(['\r', '\n'], " ");
    placement.truncate(placement.floor_char_boundary(2048));
    let repository = &row.asked.repository;
    Ok(WorkJob {
        id: ID::from(row.id),
        work_type: row.asked.work_type.as_str().to_owned(),
        kind: store::word(&row.asked.kind).map_err(internal)?,
        project_id: row.asked.project.map(ID::from),
        repository: Some(format!("{}/{}", repository.owner, repository.name)),
        state: row.state.into(),
        attempt: i32::try_from(row.attempt).map_err(internal)?,
        device_id: row.holder.or(row.last_device).map(ID::from),
        created_at: row.created_at,
        claimed_at: row.claimed_at,
        changed_at: row.changed_at,
        ended_at: row.ended_at,
        placement: Some(placement),
    })
}

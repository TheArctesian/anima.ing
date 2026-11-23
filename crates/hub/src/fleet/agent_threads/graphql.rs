use std::{collections::VecDeque, time::Duration};

use async_graphql::{Context, ID, Json, Object, Result, SimpleObject, Subscription};
use chrono::{DateTime, Utc};
use futures_util::{Stream, stream};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    model::{Entry, Filter, Summary},
    store,
};
use crate::{
    api_errors::{FieldError, internal, invalid, pool},
    fleet::access::{Authenticated, person_only},
    paging::PageEnd,
};

#[derive(SimpleObject)]
struct AgentThreadSummary {
    id: ID,
    device_id: ID,
    program: String,
    state: String,
    title: String,
    started_at: DateTime<Utc>,
    activity_at: DateTime<Utc>,
    repository: Option<String>,
}

#[derive(SimpleObject)]
struct AgentThreadPage {
    nodes: Vec<AgentThreadSummary>,
    page_info: PageEnd,
}

#[derive(SimpleObject)]
struct AgentThreadEntry {
    sequence: String,
    content: Json<serde_json::Value>,
}

#[derive(SimpleObject)]
struct AgentThreadSnapshot {
    summary: AgentThreadSummary,
    entries: Vec<AgentThreadEntry>,
    last_sequence: String,
}

#[derive(Default)]
pub struct Query;

#[Object(name = "AgentThreadsQuery")]
impl Query {
    #[graphql(guard = "Authenticated")]
    #[expect(
        clippy::too_many_arguments,
        reason = "the K1 list takes four independent filters"
    )]
    async fn agent_threads(
        &self,
        ctx: &Context<'_>,
        device: Option<ID>,
        program: Option<String>,
        project: Option<String>,
        state: Option<String>,
        #[graphql(default = 50)] first: i32,
        after: Option<String>,
    ) -> Result<AgentThreadPage> {
        let limit = usize::try_from(first)
            .ok()
            .filter(|limit| (1..=200).contains(limit))
            .ok_or_else(|| bad("first"))?;
        let filter = Filter {
            device: device
                .map(|id| id.parse().map_err(|_| bad("device")))
                .transpose()?,
            program,
            project,
            state,
            after: after.as_deref().map(cursor).transpose()?,
        };
        let mut rows = store::list(pool(ctx)?, filter, i64::from(first) + 1)
            .await
            .map_err(internal)?;
        let has_next_page = rows.len() > limit;
        rows.truncate(limit);
        Ok(AgentThreadPage {
            page_info: PageEnd {
                end_cursor: rows
                    .last()
                    .map(|row| format!("{}/{}", row.activity_at.to_rfc3339(), row.id)),
                has_next_page,
            },
            nodes: rows.into_iter().map(Into::into).collect(),
        })
    }

    #[graphql(guard = "Authenticated")]
    async fn agent_thread(&self, ctx: &Context<'_>, id: ID) -> Result<Option<AgentThreadSnapshot>> {
        person_only(ctx)?;
        let id = id.parse().map_err(|_| bad("id"))?;
        let Some((summary, entries)) = store::snapshot(pool(ctx)?, id).await.map_err(internal)?
        else {
            return Ok(None);
        };
        let last_sequence = entries.last().map_or(0, |entry| entry.sequence).to_string();
        Ok(Some(AgentThreadSnapshot {
            summary: summary.into(),
            entries: entries.into_iter().map(Into::into).collect(),
            last_sequence,
        }))
    }
}

#[derive(Default)]
pub struct Subscription;

#[Subscription(name = "AgentThreadsSubscription")]
impl Subscription {
    #[graphql(guard = "Authenticated")]
    #[expect(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "the Subscription macro requires async resolvers"
    )]
    async fn agent_thread_entries(
        &self,
        ctx: &Context<'_>,
        id: ID,
        after_sequence: String,
    ) -> Result<impl Stream<Item = Result<AgentThreadEntry>>> {
        person_only(ctx)?;
        let id = id.parse().map_err(|_| bad("id"))?;
        let sequence = after_sequence
            .parse::<i64>()
            .ok()
            .filter(|seq| (0..=(1_i64 << 53) - 1).contains(seq))
            .ok_or_else(|| bad("afterSequence"))?;
        Ok(stream::unfold(
            Some((pool(ctx)?.clone(), id, sequence, VecDeque::new())),
            next,
        ))
    }
}

type Position = (PgPool, Uuid, i64, VecDeque<Entry>);

async fn next(position: Option<Position>) -> Option<(Result<AgentThreadEntry>, Option<Position>)> {
    let (pool, id, mut sequence, mut pending) = position?;
    loop {
        if let Some(entry) = pending.pop_front() {
            sequence = entry.sequence;
            return Some((Ok(entry.into()), Some((pool, id, sequence, pending))));
        }
        match store::entries(&pool, id, sequence).await {
            Ok(rows) if rows.is_empty() => tokio::time::sleep(Duration::from_millis(500)).await,
            Ok(rows) => pending = rows.into(),
            Err(error) => return Some((Err(internal(error)), None)),
        }
    }
}

fn bad(field: &'static str) -> async_graphql::Error {
    invalid(&[FieldError::new(field, "Invalid thread query value.")])
}

fn cursor(encoded: &str) -> Result<(DateTime<Utc>, Uuid)> {
    let (time, id) = encoded.split_once('/').ok_or_else(|| bad("after"))?;
    Ok((
        time.parse().map_err(|_| bad("after"))?,
        id.parse().map_err(|_| bad("after"))?,
    ))
}

impl From<Summary> for AgentThreadSummary {
    fn from(row: Summary) -> Self {
        Self {
            id: ID(row.id.to_string()),
            device_id: ID(row.device_id.to_string()),
            program: row.program,
            state: row.state,
            title: row.title,
            started_at: row.started_at,
            activity_at: row.activity_at,
            repository: row.repository,
        }
    }
}

impl From<Entry> for AgentThreadEntry {
    fn from(row: Entry) -> Self {
        Self {
            sequence: row.sequence.to_string(),
            content: Json(row.content),
        }
    }
}

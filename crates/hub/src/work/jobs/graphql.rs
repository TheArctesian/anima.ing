mod change;
mod view;

use async_graphql::{Context, ID, Object, Result};
use daemon_core::wire::StopReason;

use super::store::admin;
use super::{Asker, Queue};
use crate::{
    api_errors::{ErrorCode, FieldError, coded, internal, invalid, pool, schema_dependency},
    fleet::{
        access::{Authenticated, person_only},
        events::{Bus, ClientAddr},
    },
    ids::JobId,
};

pub use change::Mutation;

use view::{WorkJobDetail, WorkJobPage, WorkJobState};

#[derive(Default)]
pub struct Query;

#[Object(name = "JobsQuery")]
impl Query {
    #[graphql(guard = "Authenticated")]
    async fn jobs(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 50)] first: i32,
        after: Option<String>,
        state: Option<WorkJobState>,
    ) -> Result<WorkJobPage> {
        let limit = usize::try_from(first)
            .ok()
            .filter(|value| (1..=100).contains(value))
            .ok_or_else(|| bad("first"))?;
        let after = after.as_deref().map(view::cursor).transpose()?;
        let mut tx = pool(ctx)?.begin().await.map_err(internal)?;
        let rows = admin::page(
            &mut tx,
            i64::from(first) + 1,
            after,
            state.map(WorkJobState::word),
        )
        .await
        .map_err(internal)?;
        let page = view::page(&mut tx, rows, limit).await?;
        tx.commit().await.map_err(internal)?;
        Ok(page)
    }

    #[graphql(guard = "Authenticated")]
    async fn job(&self, ctx: &Context<'_>, id: ID) -> Result<Option<WorkJobDetail>> {
        let id = id.parse::<JobId>().map_err(|_| bad("id"))?;
        let mut tx = pool(ctx)?.begin().await.map_err(internal)?;
        let detail = view::detail(&mut tx, id).await?;
        tx.commit().await.map_err(internal)?;
        Ok(detail)
    }
}

fn queue<'a>(ctx: &Context<'a>) -> Result<Queue<'a>> {
    Ok(Queue {
        db: pool(ctx)?,
        bus: schema_dependency::<Bus>(ctx)?,
    })
}

fn asker(ctx: &Context<'_>) -> Result<Asker> {
    let person = person_only(ctx)?
        .operator()
        .ok_or_else(|| coded("Person login required.", ErrorCode::Forbidden))?;
    Ok((person.clone(), *schema_dependency::<ClientAddr>(ctx)?))
}

fn bad(field: &'static str) -> async_graphql::Error {
    invalid(&[FieldError::new(field, "Invalid work value.")])
}

async fn stop(ctx: &Context<'_>, id: JobId) -> Result<bool> {
    let asker = asker(ctx)?;
    let now = schema_dependency::<std::sync::Arc<dyn crate::clock::Clock>>(ctx)?.now();
    super::stop(queue(ctx)?, (id, StopReason::Operator), asker, now)
        .await
        .map(|job| job.is_some())
        .map_err(internal)
}

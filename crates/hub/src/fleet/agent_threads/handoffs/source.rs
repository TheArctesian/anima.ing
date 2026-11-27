use super::{Error, MoveRequest, Result};
use agent_threads::managed::workspace::{Action, Capacity, Phase, View};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, types::Json};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Limits {
    memory_bytes: u64,
    cpu_millis: u64,
    tasks: u64,
}

impl Limits {
    pub(super) const fn fit(self, capacity: Capacity) -> bool {
        self.memory_bytes <= capacity.memory_bytes
            && self.cpu_millis <= capacity.cpu_millis
            && self.tasks <= capacity.tasks
    }

    pub(super) const fn start(self, generation: u64) -> Action {
        Action::Start {
            generation,
            memory_bytes: self.memory_bytes,
            cpu_millis: self.cpu_millis,
            tasks: self.tasks,
        }
    }
}

pub(super) async fn current(
    tx: &mut PgConnection,
    principal: Uuid,
    request: MoveRequest,
) -> Result<View> {
    let source: Option<Json<View>> = sqlx::query_scalar("SELECT r.receipt->'result'
        FROM agent_threads.workspace_requests r JOIN agent_threads.workspace_owners w USING(device_id,workspace)
        WHERE r.device_id=$1 AND r.workspace=$2 AND w.principal=$3 AND NOT w.deleted
        AND r.state='completed' AND r.receipt->'result' IS NOT NULL
        ORDER BY r.created_at DESC,r.request DESC LIMIT 1")
        .bind(request.source_device).bind(request.workspace).bind(principal)
        .fetch_optional(tx).await?;
    let source = source
        .map(|source| source.0)
        .filter(|source| {
            source.generation == request.generation
                && matches!(source.phase, Phase::Running | Phase::Stopped)
        })
        .ok_or(Error::Refused(
            "The owned source workspace generation is unavailable.",
        ))?;
    if let Some(checkout) = &source.checkout
        && (!checkout.ready || !checkout.source.fits())
    {
        return Err(Error::Refused("The source checkout needs reconciliation."));
    }
    Ok(source)
}

pub(super) async fn limits(
    tx: &mut PgConnection,
    principal: Uuid,
    request: MoveRequest,
) -> Result<Option<Limits>> {
    let action: Option<Json<Action>> = sqlx::query_scalar(
        "SELECT payload->'action' FROM agent_threads.workspace_requests
         WHERE device_id=$1 AND workspace=$2 AND principal=$3 AND state='completed'
           AND payload->'action'->>'kind'='start' AND payload->'action'->>'generation'=$4
           AND receipt->'result'->>'phase'='running' AND receipt->'result'->>'generation'=$4
         ORDER BY created_at DESC,request DESC LIMIT 1",
    )
    .bind(request.source_device)
    .bind(request.workspace)
    .bind(principal)
    .bind(request.generation.to_string())
    .fetch_optional(tx)
    .await?;
    Ok(action.and_then(|Json(action)| match action {
        Action::Start {
            memory_bytes,
            cpu_millis,
            tasks,
            ..
        } => Some(Limits {
            memory_bytes,
            cpu_millis,
            tasks,
        }),
        _ => None,
    }))
}

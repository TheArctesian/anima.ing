//! An opted-in move can continue the owner's last active managed session.
mod context;
mod views;
mod workspace;

use super::{Error, MoveRequest, Result, source};
use crate::ids::DeviceId;
use agent_threads::{
    control::Action,
    drive::Program,
    managed::{
        Command,
        provider_login::{Login, Provider},
    },
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, PgPool, types::Json};
use uuid::Uuid;

pub(in crate::fleet::agent_threads) use views::with_views;
pub(in crate::fleet::agent_threads) use workspace::queue_workspace;

pub(super) async fn capture(
    tx: &mut PgConnection,
    scope: MoveRequest,
    principal: Uuid,
) -> Result<()> {
    let generation =
        i64::try_from(scope.generation).map_err(|_| Error::Refused("Invalid generation."))?;
    let Some((previous, action)) = previous_command(tx, scope, principal, generation).await? else {
        return Ok(());
    };
    let provider = match action {
        Action::Start {
            program: Program::Codex,
            ..
        } => "codex",
        Action::Start {
            program: Program::ClaudeCode,
            ..
        } => "claude",
        _ => return Ok(()),
    };
    let Some(limits) = source::limits(tx, principal, scope).await? else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO agent_threads.workspace_continuations
      (transfer,previous_command,provider,agent_request,workspace_action) VALUES($1,$2,$3,$4,$5)",
    )
    .bind(scope.transfer)
    .bind(previous)
    .bind(provider)
    .bind(Uuid::now_v7())
    .bind(Json(limits.start(scope.generation + 1)))
    .execute(tx)
    .await?;
    Ok(())
}

async fn previous_command(
    tx: &mut PgConnection,
    scope: MoveRequest,
    principal: Uuid,
    generation: i64,
) -> Result<Option<(Uuid, Action)>> {
    let row: Option<(Uuid, Json<Command>, bool)> = sqlx::query_as("SELECT c.request,c.payload,
       NOT EXISTS (SELECT 1 FROM agent_threads.commands stop
          WHERE stop.device_id=c.device_id AND stop.workspace=c.workspace AND stop.generation=c.generation
            AND stop.principal=c.principal AND stop.payload->'action'->>'call'='stop'
            AND stop.payload->'action'->>'thread'=c.request::text
            AND stop.state IN ('queued','offered','unresolved','completed')
            AND COALESCE(stop.result->>'outcome','') <> 'refused')
       AND NOT EXISTS (SELECT 1 FROM agent_threads.summaries s
          WHERE s.device_id=c.device_id AND s.runner_id=c.request AND s.state='stopped')
       FROM agent_threads.commands c JOIN agent_threads.workspaces w USING(device_id,workspace)
       WHERE c.device_id=$1 AND c.workspace=$2 AND c.principal=$3 AND c.generation=$4 AND w.enabled
         AND w.generation=c.generation AND w.ready
         AND c.state='completed' AND c.result->>'outcome'='started' AND c.result->>'thread'=c.request::text
       ORDER BY c.created_at DESC,c.request DESC LIMIT 1 FOR UPDATE OF w")
        .bind(scope.source_device).bind(scope.workspace).bind(principal).bind(generation)
        .fetch_optional(tx).await?;
    Ok(row
        .filter(|(_, command, active)| *active && command.0.fits())
        .map(|(id, command, _)| (id, command.0.action)))
}

pub(in crate::fleet::agent_threads) async fn provider_ready(
    db: &PgPool,
    device: DeviceId,
    login: &Login,
    now: DateTime<Utc>,
) -> Result<()> {
    let generation =
        i64::try_from(login.generation).map_err(|_| Error::Refused("Invalid generation."))?;
    let provider = match login.provider {
        Provider::Codex => "codex",
        Provider::Claude => "claude",
    };
    sqlx::query("UPDATE agent_threads.workspace_continuations c SET provider_ready_at=$6
        FROM agent_threads.workspace_handoffs h, agent_threads.workspaces w, agent_threads.workspace_owners o
        WHERE c.transfer=h.transfer AND h.destination_device=$1 AND h.workspace=$2 AND h.principal=$3
            AND h.generation+1=$4 AND c.provider=$5 AND h.finished_at IS NOT NULL
            AND h.activation_offered_at IS NOT NULL AND h.withdraw_requested_at IS NULL
            AND w.seen_at >= $7 AND w.device_id=h.destination_device AND w.workspace=h.workspace AND w.generation=$4 AND w.enabled AND w.ready
            AND o.device_id=w.device_id AND o.workspace=w.workspace AND o.principal=h.principal AND NOT o.deleted")
        .bind(device).bind(login.workspace).bind(login.principal).bind(generation).bind(provider)
        .bind(now).bind(now - TimeDelta::seconds(30)).execute(db).await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct Launch {
    workspace: Uuid,
    principal: Uuid,
    generation: i64,
    agent_request: Uuid,
    previous: Json<Command>,
    thread: Option<Uuid>,
}

pub(in crate::fleet::agent_threads) async fn queue_agent(
    tx: &mut PgConnection,
    device: DeviceId,
    now: DateTime<Utc>,
) -> std::result::Result<(), super::super::managed::Error> {
    let launch: Option<Launch> = sqlx::query_as("SELECT h.workspace,h.principal,w.generation,p.agent_request,
           source.payload AS previous,s.id AS thread
        FROM agent_threads.workspace_continuations p JOIN agent_threads.workspace_handoffs h USING(transfer)
        JOIN agent_threads.workspace_owners o ON o.device_id=h.destination_device AND o.workspace=h.workspace
        JOIN agent_threads.workspaces w ON w.device_id=o.device_id AND w.workspace=o.workspace
        JOIN agent_threads.workspace_requests r ON r.request=p.workspace_request AND r.device_id=w.device_id
        JOIN agent_threads.commands source ON source.request=p.previous_command AND source.device_id=h.source_device
        LEFT JOIN agent_threads.summaries s ON s.device_id=source.device_id AND s.runner_id=source.request
        WHERE h.destination_device=$1 AND h.finished_at IS NOT NULL AND h.withdraw_requested_at IS NULL
          AND h.activation_offered_at IS NOT NULL
          AND o.principal=h.principal AND NOT o.deleted AND w.enabled AND w.ready AND w.generation=h.generation+1
          AND w.seen_at >= $2 AND p.provider_ready_at IS NOT NULL
          AND r.state='completed' AND r.receipt->'result'->>'phase'='running'
          AND r.receipt->'result'->>'generation'=w.generation::text
          AND NOT EXISTS (SELECT 1 FROM agent_threads.workspace_requests later
              WHERE later.device_id=w.device_id AND later.workspace=w.workspace
              AND (later.created_at,later.request)>(r.created_at,r.request))
          AND NOT EXISTS (SELECT 1 FROM agent_threads.workspace_handoffs moving
              WHERE moving.workspace=h.workspace AND moving.finished_at IS NULL)
          AND source.principal=h.principal AND source.workspace=h.workspace AND source.generation=h.generation
          AND NOT EXISTS (SELECT 1 FROM agent_threads.commands c
              WHERE c.device_id=w.device_id AND c.workspace=w.workspace AND c.generation=w.generation AND c.principal=h.principal)
        ORDER BY h.finished_at,h.transfer LIMIT 1 FOR UPDATE OF w")
        .bind(device).bind(now - TimeDelta::seconds(30)).fetch_optional(&mut *tx).await?;
    let Some(launch) = launch else {
        return Ok(());
    };
    if claimed(tx, device, &launch).await? {
        return Ok(());
    }
    let mut action = launch.previous.0.action;
    let Action::Start { prompt, .. } = &mut action else {
        return Ok(());
    };
    *prompt = context::prompt(&mut *tx, launch.thread, prompt).await?;
    let command = Command {
        request: launch.agent_request,
        workspace: launch.workspace,
        principal: launch.principal,
        generation: u64::try_from(launch.generation)?,
        action,
    };
    match super::super::managed::enqueue_continuation(tx, device, &command, now).await {
        Ok(_) | Err(super::super::managed::Error::Refused(_)) => Ok(()),
        Err(error) => Err(error),
    }
}

async fn claimed(
    tx: &mut PgConnection,
    device: DeviceId,
    launch: &Launch,
) -> std::result::Result<bool, sqlx::Error> {
    // Recheck after taking the managed workspace row lock. A person's start may have just committed.
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_threads.commands
        WHERE device_id=$1 AND workspace=$2 AND generation=$3 AND principal=$4)",
    )
    .bind(device)
    .bind(launch.workspace)
    .bind(launch.generation)
    .bind(launch.principal)
    .fetch_one(tx)
    .await
}

pub(in crate::fleet::agent_threads) async fn pending_agent(
    tx: &mut PgConnection,
    device: DeviceId,
    command: &Command,
) -> std::result::Result<bool, super::super::managed::Error> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_threads.workspace_continuations p
        JOIN agent_threads.commands active ON active.request=p.agent_request
        WHERE active.device_id=$1 AND active.workspace=$2 AND active.generation=$3
        AND active.request<>$4 AND active.state IN ('queued','offered','unresolved'))",
    )
    .bind(device)
    .bind(command.workspace)
    .bind(i64::try_from(command.generation)?)
    .bind(command.request)
    .fetch_one(tx)
    .await?)
}

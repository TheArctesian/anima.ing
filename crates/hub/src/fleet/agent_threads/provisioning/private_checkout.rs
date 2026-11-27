//! Attach the connected repository only to a first start backed by a prepared workspace.
use agent_threads::managed::workspace::{Action, Command};
use sqlx::{PgConnection, types::Json};

use super::{Error, Result};
use crate::{
    ids::DeviceId,
    work::checkout::workspace::{self, Binding},
};

pub(super) async fn for_command(
    conn: &mut PgConnection,
    device: DeviceId,
    command: &Command,
    capable: bool,
) -> Result<Option<Binding>> {
    let binding = match &command.action {
        Action::Prepare {
            source: Some(source),
            ..
        } if source.repository_id.is_some() => Some(workspace::bind(conn, source).await?.ok_or(
            Error::Refused("Choose an active private repository from a connected GitHub account."),
        )?),
        Action::Start { generation: 1, .. } => prepared_source(conn, device, command).await?,
        _ => None,
    };
    if binding.is_some() && !capable {
        return Err(Error::Refused(
            "This node does not support private workspace checkout.",
        ));
    }
    Ok(binding)
}

async fn prepared_source(
    conn: &mut PgConnection,
    device: DeviceId,
    command: &Command,
) -> Result<Option<Binding>> {
    let row: Option<(Json<Binding>, bool)> = sqlx::query_as(
        "SELECT p.checkout_binding, COALESCE(
           v.receipt #>> '{result,phase}'='prepared'
           AND v.receipt #> '{result,generation}'='0'::jsonb
           AND v.receipt #> '{result,checkout,ready}'='false'::jsonb
           AND v.receipt #> '{result,checkout,source}'=p.payload #> '{action,source}'
           AND NOT EXISTS (
               SELECT 1 FROM agent_threads.workspace_requests s
               JOIN agent_threads.workspace_checkout_releases x ON x.request=s.request
               WHERE s.device_id=p.device_id AND s.workspace=p.workspace AND s.principal=p.principal
                 AND (s.created_at,s.request)>(v.created_at,v.request) AND x.release_until IS NOT NULL
           ), false)
         FROM agent_threads.workspace_requests p
         JOIN agent_threads.workspace_owners o USING (device_id,workspace)
         LEFT JOIN LATERAL (
           SELECT request,created_at,receipt FROM agent_threads.workspace_requests
           WHERE device_id=p.device_id AND workspace=p.workspace AND principal=p.principal
             AND state='completed' AND created_at>=o.created_at
           ORDER BY created_at DESC,request DESC LIMIT 1
         ) v ON true
         WHERE p.device_id=$1 AND p.workspace=$2 AND p.principal=$3 AND p.created_at>=o.created_at
           AND p.checkout_binding IS NOT NULL AND p.payload->'action'->>'kind'='prepare'
         ORDER BY p.created_at DESC,p.request DESC LIMIT 1",
    )
    .bind(device).bind(command.workspace).bind(command.principal).fetch_optional(conn).await?;
    match row {
        Some((Json(binding), true)) => Ok(Some(binding)),
        Some((_, false)) => Err(Error::Refused(
            "Reconcile the private workspace before starting checkout.",
        )),
        None => Ok(None),
    }
}

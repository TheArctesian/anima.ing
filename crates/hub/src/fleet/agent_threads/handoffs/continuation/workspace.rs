use crate::fleet::agent_threads::provisioning::{self, Error};
use crate::ids::DeviceId;
use agent_threads::managed::workspace::{Action, Capacity, Command};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, types::Json};
use uuid::Uuid;

pub(in crate::fleet::agent_threads) async fn queue_workspace(
    tx: &mut PgConnection,
    device: DeviceId,
    capacity: Capacity,
    now: DateTime<Utc>,
) -> std::result::Result<(), Error> {
    let next: Option<(Uuid, Uuid, Uuid, Json<Action>)> = sqlx::query_as(
        "SELECT c.transfer,h.workspace,h.principal,c.workspace_action
         FROM agent_threads.workspace_continuations c JOIN agent_threads.workspace_handoffs h USING(transfer)
         JOIN agent_threads.workspace_owners o ON o.device_id=h.destination_device AND o.workspace=h.workspace
         JOIN LATERAL (
            SELECT r.state,r.receipt,r.created_at FROM agent_threads.workspace_requests r
            WHERE r.device_id=o.device_id AND r.workspace=o.workspace
            ORDER BY r.created_at DESC,r.request DESC LIMIT 1
         ) latest ON true
         WHERE h.destination_device=$1 AND h.finished_at IS NOT NULL AND h.withdraw_requested_at IS NULL
           AND h.activation_offered_at IS NOT NULL AND NOT o.deleted AND o.principal=h.principal
           AND c.workspace_request IS NULL
           AND latest.created_at=h.finished_at AND latest.state='completed'
           AND latest.receipt->'result'->>'phase'='prepared'
           AND latest.receipt->'result'->>'generation'=h.generation::text
           AND (c.workspace_action->>'memory_bytes')::bigint <= ($2::jsonb->>'memoryBytes')::bigint
           AND (c.workspace_action->>'cpu_millis')::bigint <= ($2::jsonb->>'cpuMillis')::bigint
           AND (c.workspace_action->>'tasks')::bigint <= ($2::jsonb->>'tasks')::bigint
         ORDER BY h.finished_at,h.transfer LIMIT 1",
    )
    .bind(device)
    .bind(Json(capacity))
    .fetch_optional(&mut *tx)
    .await?;
    let Some((transfer, workspace, principal, Json(action))) = next else {
        return Ok(());
    };
    let command = Command {
        request: Uuid::now_v7(),
        workspace,
        principal,
        action,
    };
    match provisioning::enqueue_continuation(tx, device, &command, now).await {
        Ok(_) => {
            sqlx::query("UPDATE agent_threads.workspace_continuations SET workspace_request=$2 WHERE transfer=$1")
                .bind(transfer).bind(command.request).execute(tx).await?;
            Ok(())
        }
        Err(Error::Refused(_)) => Ok(()),
        Err(error) => Err(error),
    }
}

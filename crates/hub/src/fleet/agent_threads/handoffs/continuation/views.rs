use crate::ids::DeviceId;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

pub(in crate::fleet::agent_threads) async fn with_views(
    db: &PgPool,
    device: DeviceId,
    principal: Uuid,
    mut rows: Vec<Value>,
) -> Result<Vec<Value>, sqlx::Error> {
    for (workspace, generation, continuation) in views(db, device, principal).await? {
        if let Some(row) = rows.iter_mut().find(|row| {
            row["workspace"].as_str().and_then(|id| id.parse().ok()) == Some(workspace)
                && row["generation"].as_i64() == Some(generation)
        }) {
            row["continuation"] = continuation;
        }
    }
    Ok(rows)
}

async fn views(
    db: &PgPool,
    device: DeviceId,
    principal: Uuid,
) -> Result<Vec<(Uuid, i64, Value)>, sqlx::Error> {
    sqlx::query_as("SELECT w.workspace,w.generation,jsonb_build_object(
        'request',c.agent_request,'provider',c.provider,'sourceThread',s.id,'delivery',a.state,
        'signedIn',c.provider_ready_at IS NOT NULL)
        FROM agent_threads.workspace_continuations c JOIN agent_threads.workspace_handoffs h USING(transfer)
        JOIN agent_threads.workspace_owners o ON o.device_id=h.destination_device AND o.workspace=h.workspace
        JOIN agent_threads.workspaces w ON w.device_id=o.device_id AND w.workspace=o.workspace
        LEFT JOIN agent_threads.summaries s ON s.device_id=h.source_device AND s.runner_id=c.previous_command
        LEFT JOIN agent_threads.commands a ON a.request=c.agent_request
        WHERE w.device_id=$1 AND h.principal=$2 AND w.generation=h.generation+1
            AND o.principal=$2 AND NOT o.deleted AND h.finished_at IS NOT NULL
            AND h.activation_offered_at IS NOT NULL AND h.withdraw_requested_at IS NULL
            AND EXISTS (SELECT 1 FROM agent_threads.workspace_requests r
                WHERE r.request=c.workspace_request AND r.state='completed'
                AND r.receipt->'result'->>'phase'='running')
            AND (a.request IS NOT NULL OR NOT EXISTS (SELECT 1 FROM agent_threads.commands other
                WHERE other.device_id=w.device_id AND other.workspace=w.workspace AND other.generation=w.generation
                  AND other.principal=h.principal))
        ORDER BY w.workspace LIMIT 128")
        .bind(device).bind(principal).fetch_all(db).await
}

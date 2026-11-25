use sqlx::PgPool;
use uuid::Uuid;

use super::{Error, Owner, attempt, store};

pub(in crate::fleet::agent_threads) async fn sweep(db: &PgPool) -> Result<(), Error> {
    let devices: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT launch.device_id FROM agent_threads.launches launch
         JOIN devices.devices device ON device.id = launch.device_id
         WHERE (launch.deadline <= clock_timestamp() AND launch.publication IS NULL
                AND (launch.authorization IS NOT NULL OR (launch.claimed_at IS NULL AND launch.evidence IS NULL)))
            OR (launch.lease_end IS NOT NULL AND
                least(launch.lease_end, launch.claimed_at + interval '120 seconds') <= clock_timestamp())
            OR EXISTS (SELECT FROM agent_threads.commands command
                       WHERE command.device_id = launch.device_id AND command.state = 'waiting'
                         AND command.deadline <= clock_timestamp())
         ORDER BY launch.device_id",
    )
    .fetch_all(db)
    .await?;
    for device in devices {
        sweep_device(db, device).await?;
    }
    retire(db).await
}

async fn sweep_device(db: &PgPool, device: Uuid) -> Result<(), Error> {
    let mut tx = db.begin().await?;
    store::lock_existing(&mut tx, device).await?;
    let at = store::now(&mut tx).await?;
    sqlx::query(
        "UPDATE agent_threads.launches SET lease_end = NULL, attempt_phase = 'expired'
         WHERE device_id = $1 AND
         (lease_end <= $2 OR (lease_end IS NOT NULL AND claimed_at + interval '120 seconds' <= $2))",
    )
    .bind(device)
    .bind(at)
    .execute(&mut *tx)
    .await?;
    let due: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT account_id, launch_id FROM agent_threads.launches
         WHERE device_id = $1 AND deadline <= $2 AND publication IS NULL
           AND (authorization IS NOT NULL OR (claimed_at IS NULL AND evidence IS NULL))",
    )
    .bind(device)
    .bind(at)
    .fetch_all(&mut *tx)
    .await?;
    for (account, launch) in due {
        attempt::reconcile(&mut tx, Owner { account, device }, launch, at).await?;
    }
    store::expire_waiting(&mut tx, device, at).await?;
    tx.commit().await?;
    Ok(())
}

async fn retire(db: &PgPool) -> Result<(), Error> {
    let mut tx = db.begin().await?;
    sqlx::query(
        "UPDATE agent_threads.control_offers SET offer = NULL
         WHERE offer IS NOT NULL AND expires_at <= clock_timestamp()",
    )
    .execute(&mut *tx)
    .await?;
    // Retain the retry identity. Only records already closed to delivery can retire.
    sqlx::query(
        "UPDATE agent_threads.commands SET state = 'retired', cause = NULL
         WHERE state IN ('recorded','refused','expired')
           AND closed_at < clock_timestamp() - interval '24 hours'",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE agent_threads.launches launch SET retired = true, folder = '', project = ''
         WHERE NOT retired AND launch.evidence IS NOT NULL
           AND (launch.evidence <> 'created' OR launch.withdrawn OR launch.control_closed
                OR launch.publication = 'timed_out')
           AND launch.deadline < clock_timestamp() - interval '24 hours'
           AND NOT EXISTS (SELECT FROM agent_threads.commands c WHERE c.account_id = launch.account_id
                           AND c.launch_id = launch.launch_id AND c.state <> 'retired')",
    ).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

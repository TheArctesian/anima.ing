use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::fleet::events::AuditAction;

use super::{Actor, Error, LaunchReceipt, Owner, Pair, Start, Target, audit, store};

pub(super) const KEY: i16 = 1;

pub(in crate::fleet) async fn start(
    db: &PgPool,
    owner: Owner,
    request: Start,
    actor: Actor,
) -> Result<LaunchReceipt, Error> {
    if !request.valid() {
        return Err(Error::malformed());
    }
    let mut tx = db.begin().await?;
    store::lock_id(&mut tx, owner.account, &request.launch.to_string()).await?;
    let receipt = admit_start(&mut tx, owner, &request, actor).await?;
    tx.commit().await?;
    Ok(receipt)
}

async fn admit_start(
    conn: &mut PgConnection,
    owner: Owner,
    request: &Start,
    actor: Actor,
) -> Result<LaunchReceipt, Error> {
    if let Some(receipt) = retry(conn, owner, request).await? {
        return Ok(receipt);
    }
    store::lock_device(conn, owner.device).await?;
    let at = store::now(conn).await?;
    store::expire_waiting(conn, owner.device, at).await?;
    store::capacity(conn, owner.device, true).await?;
    if store::blocked(conn, owner.device, at).await?
        || !eligible(conn, owner, &request.pair, at).await?
    {
        return Err(Error::Refused("target_unavailable"));
    }
    store::reserve_record(conn, KEY).await?;
    store::reserve_record(conn, KEY).await?;
    let receipt = insert_start(conn, owner, request, at).await?;
    audit::write(
        conn,
        (owner, at),
        actor.clone(),
        (
            AuditAction::Start,
            json!({
                "launch": receipt.launch, "action": "launch", "outcome": "queued",
            }),
        ),
    )
    .await?;
    audit::write(
        conn,
        (owner, at),
        actor,
        (
            AuditAction::Create,
            json!({
                "launch": receipt.launch, "command": receipt.first_command,
                "action": "prompt", "outcome": "waiting",
            }),
        ),
    )
    .await?;
    Ok(receipt)
}

async fn retry(
    conn: &mut PgConnection,
    owner: Owner,
    request: &Start,
) -> Result<Option<LaunchReceipt>, Error> {
    let prior = match store::launch(conn, owner.account, request.launch).await {
        Ok(prior) => prior,
        Err(Error::Refused("LAUNCH_UNKNOWN")) => return Ok(None),
        Err(error) => return Err(error),
    };
    let digest =
        store::fingerprint(conn, prior.key_version, "launch", &(owner.device, request)).await?;
    if prior.fingerprint != digest {
        return Err(Error::Refused("LAUNCH_CONFLICT"));
    }
    store::lock_existing(conn, owner.device).await?;
    let at = store::now(conn).await?;
    super::attempt::reconcile(conn, owner, request.launch, at)
        .await
        .map(Some)
}

async fn insert_start(
    conn: &mut PgConnection,
    owner: Owner,
    request: &Start,
    at: DateTime<Utc>,
) -> Result<LaunchReceipt, Error> {
    let fingerprint = store::fingerprint(conn, KEY, "launch", &(owner.device, request)).await?;
    let (first, work) = (Uuid::now_v7(), Uuid::now_v7());
    sqlx::query(
        "INSERT INTO agent_threads.launches (account_id, launch_id, work_id, device_id,
            first_command, fingerprint, key_version, project, folder, program, access, admitted_at, deadline)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
    )
    .bind(owner.account)
    .bind(request.launch)
    .bind(work)
    .bind(owner.device)
    .bind(first)
    .bind(fingerprint)
    .bind(KEY)
    .bind(&request.pair.project)
    .bind(&request.pair.folder)
    .bind(request.pair.program.word())
    .bind(&request.access)
    .bind(at)
    .bind(at + store::TEN_MINUTES)
    .execute(&mut *conn)
    .await?;
    super::commands::insert_first(conn, (owner, request.launch, first), &request.prompt, at)
        .await?;
    store::launch(conn, owner.account, request.launch)
        .await
        .map(|row| row.receipt(at))
}

pub(in crate::fleet) async fn launch_receipt(
    db: &PgPool,
    account: Uuid,
    id: Uuid,
) -> Result<LaunchReceipt, Error> {
    let device = store::launch_device(db, account, id).await?;
    let mut tx = db.begin().await?;
    store::lock_device(&mut tx, device).await?;
    let at = store::now(&mut tx).await?;
    let receipt = super::attempt::reconcile(&mut tx, Owner { account, device }, id, at).await?;
    tx.commit().await?;
    Ok(receipt)
}

pub(in crate::fleet) async fn targets(db: &PgPool, account: Uuid) -> Result<Vec<Target>, Error> {
    let rows: Vec<(Uuid, Value, DateTime<Utc>)> = sqlx::query_as(
        "SELECT offers.device_id, offers.pairs, offers.expires_at
         FROM agent_threads.start_offers offers
         JOIN devices.devices device ON device.id = offers.device_id AND device.metrics_source = 'daemon'
         JOIN metrics.daemons enrolled ON enrolled.device_id = device.id
         LEFT JOIN jobs.machines work ON work.device_id = device.id
         CROSS JOIN (SELECT clock_timestamp() AS at) clock
         WHERE offers.account_id = $1 AND offers.expires_at > clock.at AND enrolled.expires_at > clock.at
           AND NOT COALESCE(work.off_by_operator OR work.off_until > clock.at, false)
         ORDER BY offers.device_id",
    )
    .bind(account)
    .fetch_all(db)
    .await?;
    let mut targets = Vec::new();
    for (device, pairs, expires_at) in rows {
        targets.extend(
            serde_json::from_value::<Vec<Pair>>(pairs)?
                .into_iter()
                .map(|pair| Target {
                    device,
                    pair,
                    expires_at,
                }),
        );
    }
    Ok(targets)
}

pub(super) async fn eligible(
    conn: &mut PgConnection,
    owner: Owner,
    pair: &Pair,
    at: DateTime<Utc>,
) -> Result<bool, Error> {
    let offered: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT FROM agent_threads.start_offers
            WHERE device_id = $1 AND account_id = $2 AND expires_at > $3 AND pairs @> $4
        )",
    )
    .bind(owner.device)
    .bind(owner.account)
    .bind(at)
    .bind(json!([pair]))
    .fetch_one(&mut *conn)
    .await?;
    Ok(offered && store::credential_is_live(conn, owner.device, at).await?)
}

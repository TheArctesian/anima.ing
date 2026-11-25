mod claim_reply;
mod evidence;
mod reports;

use chrono::{DateTime, TimeDelta, Utc};
use serde_json::json;
use sqlx::PgConnection;
use uuid::Uuid;

use claim_reply::ThreadWork;
pub(in crate::fleet) use evidence::evidence;
pub(in crate::fleet::agent_threads) use evidence::{published, reconcile};
pub(in crate::fleet) use reports::report;

use super::{
    Dispatch, DispatchDecision, Error, Launch, OriginalClaim, Owner, Pair, admission, grant, store,
};

const LEASE: TimeDelta = TimeDelta::seconds(90);
pub(super) const MAX_ATTEMPT: TimeDelta = TimeDelta::seconds(120);

pub(in crate::fleet) async fn replace_targets(
    conn: &mut PgConnection,
    owner: Owner,
    (pairs, enabled): (&[Pair], bool),
    at: DateTime<Utc>,
) -> Result<(), Error> {
    if pairs.iter().any(|pair| !pair.valid()) {
        return Err(Error::malformed());
    }
    sqlx::query(
        "INSERT INTO agent_threads.start_offers (device_id, account_id, pairs, expires_at, enabled)
         VALUES ($1,$2,$3,$4,$5) ON CONFLICT (device_id) DO UPDATE SET
         account_id = EXCLUDED.account_id, pairs = EXCLUDED.pairs, expires_at = EXCLUDED.expires_at,
         enabled = EXCLUDED.enabled",
    )
    .bind(owner.device)
    .bind(owner.account)
    .bind(serde_json::to_value(pairs)?)
    .bind(at + store::FRESH)
    .bind(enabled)
    .execute(conn)
    .await?;
    Ok(())
}

pub(in crate::fleet) async fn candidate(
    conn: &mut PgConnection,
    owner: Owner,
    holds: &[daemon_core::wire::Held],
    at: DateTime<Utc>,
) -> Result<Option<(Uuid, DateTime<Utc>)>, Error> {
    let held: Option<(Uuid,)> = sqlx::query_as(
        "SELECT work_id FROM agent_threads.launches WHERE account_id = $1 AND device_id = $2
         AND device_removed_at IS NULL AND (evidence IS NULL OR evidence = 'created')
         AND lease_end > $3 AND claimed_at + interval '120 seconds' > $3
         AND deadline > $3",
    )
    .bind(owner.account)
    .bind(owner.device)
    .bind(at)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some((work,)) = held {
        if holds
            .iter()
            .any(|item| item.id == work && item.attempt == 1)
        {
            return Ok(None);
        }
        sqlx::query("UPDATE agent_threads.launches SET attempt_phase = 'expired', lease_end = NULL WHERE work_id = $1")
            .bind(work)
            .execute(&mut *conn)
            .await?;
    }
    let waiting: Vec<(Uuid, DateTime<Utc>)> = sqlx::query_as(
        "SELECT launch_id, admitted_at FROM agent_threads.launches
         WHERE account_id = $1 AND device_id = $2 AND attempt_phase IS NULL AND evidence IS NULL
           AND deadline > $3 AND device_removed_at IS NULL
         ORDER BY admitted_at, work_id LIMIT 31",
    )
    .bind(owner.account)
    .bind(owner.device)
    .bind(at)
    .fetch_all(&mut *conn)
    .await?;
    for (launch, admitted_at) in waiting {
        let row = store::launch(conn, owner.account, launch).await?;
        if still_offered(conn, owner, &row, at).await? {
            return Ok(Some((launch, admitted_at)));
        }
        evidence::never_claimed(conn, (owner, &row), "target_unavailable").await?;
    }
    Ok(None)
}

pub(in crate::fleet) async fn claim(
    conn: &mut PgConnection,
    owner: Owner,
    launch: Uuid,
    at: DateTime<Utc>,
) -> Result<ThreadWork, Error> {
    let row = store::launch(conn, owner.account, launch).await?;
    let end = at + LEASE;
    sqlx::query(
        "UPDATE agent_threads.launches SET attempt_phase = 'claimed', claimed_at = $3, lease_end = $4
         WHERE account_id = $1 AND launch_id = $2",
    )
    .bind(owner.account)
    .bind(row.id)
    .bind(at)
    .bind(end)
    .execute(conn)
    .await?;
    ThreadWork::for_launch(&row, owner.account, end)
}

async fn still_offered(
    conn: &mut PgConnection,
    owner: Owner,
    row: &Launch,
    at: DateTime<Utc>,
) -> Result<bool, Error> {
    let pair = serde_json::from_value(json!({
        "project": row.project,
        "folder": row.folder,
        "program": row.program,
    }))?;
    admission::eligible(conn, owner, &pair, at).await
}

async fn original(
    conn: &mut PgConnection,
    owner: Owner,
    request: OriginalClaim,
) -> Result<Launch, Error> {
    if request.launch.is_nil() || request.work.is_nil() || request.attempt == 0 {
        return Err(Error::malformed());
    }
    if request.device != owner.device {
        return Err(Error::Refused("WRONG_DEVICE"));
    }
    let launch = store::launch(conn, owner.account, request.launch).await?;
    if launch.device_id != owner.device || launch.device_removed_at.is_some() {
        return Err(Error::Refused("LAUNCH_UNKNOWN"));
    }
    if request.work != launch.work_id || request.attempt != 1 {
        return Err(Error::Refused("LAUNCH_CONFLICT"));
    }
    if launch.claimed_at.is_none() {
        return Err(Error::Refused("LAUNCH_UNKNOWN"));
    }
    Ok(launch)
}

pub(in crate::fleet) async fn dispatch(
    conn: &mut PgConnection,
    owner: Owner,
    request: Dispatch,
    at: DateTime<Utc>,
) -> Result<DispatchDecision, Error> {
    if request.runner.is_nil() {
        return Err(Error::malformed());
    }
    let row = original(conn, owner, request.claim).await?;
    if row.runner_id.is_some_and(|runner| runner != request.runner) {
        return Err(Error::Refused("LAUNCH_CONFLICT"));
    }
    if let Some(denial) = row.denial.as_deref() {
        return Ok(DispatchDecision::Denied(match denial {
            "WORK_OFF" => "WORK_OFF",
            "target_unavailable" => "target_unavailable",
            _ => "LAUNCH_CONFLICT",
        }));
    }
    if row.evidence.is_some() {
        return Ok(DispatchDecision::Reconcile(row.authorization));
    }
    if row.authorization.is_some() {
        return Ok(grant_reply(&row, at));
    }
    let Some(lease) = live_lease(&row, at) else {
        return Ok(DispatchDecision::Reconcile(None));
    };
    let reason = denial(conn, owner, (&row, request.runner), at).await?;
    if let Some(reason) = reason {
        record_denial(conn, owner, (row.id, request.runner, reason)).await?;
        return Ok(DispatchDecision::Denied(reason));
    }
    let Some(sample) = grant::tick() else {
        return Ok(DispatchDecision::Reconcile(None));
    };
    let issued = store::now(conn).await?;
    let until = grant::expiry(issued, lease, row.deadline);
    if until <= issued {
        return Ok(DispatchDecision::Reconcile(None));
    }
    record_grant(
        conn,
        owner,
        (row.id, request.runner),
        (sample, issued, until),
    )
    .await?;
    let row = store::launch(conn, owner.account, row.id).await?;
    Ok(grant_reply(&row, store::now(conn).await?))
}

fn grant_reply(row: &Launch, at: DateTime<Utc>) -> DispatchDecision {
    live_lease(row, at)
        .and_then(|_| grant::tick())
        .and_then(|sample| grant::remaining(row, at, sample))
        .map_or(
            DispatchDecision::Reconcile(row.authorization),
            DispatchDecision::Granted,
        )
}

fn live_lease(row: &Launch, at: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let end = row.lease_end?.min(row.claimed_at? + MAX_ATTEMPT);
    (end > at && at < row.deadline).then_some(end)
}

async fn denial(
    conn: &mut PgConnection,
    owner: Owner,
    (row, runner): (&Launch, Uuid),
    at: DateTime<Utc>,
) -> Result<Option<&'static str>, Error> {
    if store::blocked(conn, owner.device, at).await? {
        return Ok(Some("WORK_OFF"));
    }
    let used: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT FROM agent_threads.summaries WHERE device_id = $1 AND runner_id = $2)
             OR EXISTS (SELECT FROM agent_threads.launches
                        WHERE device_id = $1 AND runner_id = $2 AND \"authorization\" IS NOT NULL)",
    )
    .bind(owner.device)
    .bind(runner)
    .fetch_one(&mut *conn)
    .await?;
    if used {
        return Ok(Some("LAUNCH_CONFLICT"));
    }
    Ok((!still_offered(conn, owner, row, at).await?).then_some("target_unavailable"))
}

async fn record_denial(
    conn: &mut PgConnection,
    owner: Owner,
    (launch, runner, reason): (Uuid, Uuid, &str),
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE agent_threads.launches SET runner_id = $3, denial = $4
         WHERE account_id = $1 AND launch_id = $2",
    )
    .bind(owner.account)
    .bind(launch)
    .bind(runner)
    .bind(reason)
    .execute(conn)
    .await?;
    Ok(())
}

async fn record_grant(
    conn: &mut PgConnection,
    owner: Owner,
    (launch, runner): (Uuid, Uuid),
    (sample, issued, valid_until): (grant::Tick, DateTime<Utc>, DateTime<Utc>),
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE agent_threads.launches SET runner_id = $3, \"authorization\" = gen_random_uuid(),
                clock_generation = $4, issued_tick = $5, issued_at = $6, valid_until = $7
         WHERE account_id = $1 AND launch_id = $2",
    )
    .bind(owner.account)
    .bind(launch)
    .bind(runner)
    .bind(sample.generation)
    .bind(sample.nanoseconds)
    .bind(issued)
    .bind(valid_until)
    .execute(conn)
    .await?;
    Ok(())
}

//! A holder's report (K10 Section 7), and the end of leases that ran out and attempts too long.

use chrono::{DateTime, Utc};
use daemon_core::wire::{Lease, Reported, StopOrder, WorkKind, WorkReport, WorkState};
use sqlx::PgConnection;

use super::{
    Error, Queue, Reporting, ended,
    lifecycle::{self, Refusal},
    model::{Fault, Job, State},
    placement, results, store, trail,
};
use crate::{
    fleet::events::{Bus, ClientAddr},
    ids::{DeviceId, JobId},
};

/// Answers the report of `device` at `now` (K10 Section 7).
///
/// # Errors
/// `Database` or `Events` when a change cannot be written; nothing is changed then.
pub async fn report(
    queue: Queue<'_>,
    device: DeviceId,
    (report, artifact_retained): (&WorkReport, bool),
    (client, now): (ClientAddr, DateTime<Utc>),
) -> Result<Reporting, Error> {
    let mut tx = queue.db.begin().await?;
    let outcome = report_in(
        &mut tx,
        queue.bus,
        (device, report, artifact_retained),
        (client, now),
    )
    .await;
    ended(tx, outcome).await
}

/// Records the report in the caller's transaction, which can also hold daemon authentication locks.
pub(crate) async fn report_in(
    conn: &mut PgConnection,
    bus: &Bus,
    (device, report, artifact_retained): (DeviceId, &WorkReport, bool),
    (client, now): (ClientAddr, DateTime<Utc>),
) -> Result<Reporting, Error> {
    let work_off = store::machine_locked(conn, device)
        .await?
        .is_some_and(|machine| placement::is_off(&machine, now));
    let Some(job) = store::job_locked(conn, JobId::from(report.id)).await? else {
        return Ok(if work_off {
            Reporting::Off
        } else {
            Reporting::NotHolder
        });
    };
    let host = job.asked.kind == WorkKind::Host;
    if work_off && !host {
        return Ok(Reporting::Off);
    }
    if let Some(answer) = final_replay(&job, device, report) {
        return Ok(Reporting::Reported(answer));
    }
    let mut next = match lifecycle::reported(&job, device, report, now) {
        Ok(next) => next,
        Err(Refusal::NotHolder) => return Ok(Reporting::NotHolder),
        Err(Refusal::Malformed) => return Ok(Reporting::Malformed),
    };
    if next.state == State::Done
        && job.asked.parameters["recipe"] == "source_image_v1"
        && (!artifact_retained || !next.result.as_ref().is_some_and(source_succeeded))
    {
        return Ok(Reporting::Malformed);
    }
    if let Some(part) = &report.log
        && lifecycle::log_fits(&job, part.from, part.text.len())
    {
        store::append_log(conn, job.id, part.from, &trail::clean_log(&part.text)).await?;
        next.log_end = job.log_end + part.text.len() as u64;
    }
    store::save(conn, &next).await?;
    if report.state == WorkState::Refused {
        if counts_toward_cooldown(job.asked.kind) {
            fault(conn, bus, (device, Fault::Refused), now).await?;
        }
        trail::attempt_ended(conn, &next, (device, "refused"), (client, now)).await?;
    } else if next.state.is_final() {
        trail::attempt_ended(conn, &next, (device, next.state.as_str()), (client, now)).await?;
    }
    Ok(Reporting::Reported(Reported {
        lease: Lease {
            until: next.lease_ends_at.unwrap_or(now),
        },
        stop: next.stop.map(|reason| StopOrder { reason }),
        log_end: Some(next.log_end),
    }))
}

fn counts_toward_cooldown(kind: WorkKind) -> bool {
    matches!(kind, WorkKind::Container | WorkKind::Native)
}

fn source_succeeded(result: &daemon_core::wire::WorkResult) -> bool {
    result.outcome == "succeeded"
        && result.cleanup.isolation == "deleted"
        && result.cleanup.checkout == "deleted"
}

// An owner that lost the acknowledgement may repeat its final report without replaying the work.
fn final_replay(job: &Job, device: DeviceId, report: &WorkReport) -> Option<Reported> {
    let host = job.asked.kind == WorkKind::Host;
    if (!host && job.asked.parameters["recipe"] != "source_image_v1")
        || job.last_device != Some(device)
        || job.attempt != report.attempt
        || !job.state.is_final()
        || lifecycle::reported_state(report.state, host) != Some(job.state)
        || report.log.is_some()
    {
        return None;
    }
    let sent = report.result.as_ref()?;
    let accepted = if host {
        results::host(sent, report.state, job.asked.work_type)?
    } else {
        results::checked(sent)?
    };
    if job.result.as_ref() != Some(&accepted) {
        return None;
    }
    Some(Reported {
        lease: Lease {
            until: job.ended_at?,
        },
        stop: None,
        log_end: Some(job.log_end),
    })
}

/// Counts a job that the machine refused or lost; the fifth in 1 hour gives the machine no
/// work for 1 hour, with an event (K10 Section 11, SF-011).
async fn fault(
    conn: &mut PgConnection,
    bus: &Bus,
    (device, fault): (DeviceId, Fault),
    now: DateTime<Utc>,
) -> Result<(), Error> {
    let recent =
        store::add_fault(conn, (device, fault), now, now - placement::FAULT_WINDOW).await?;
    if let Some(until) = placement::off_until(recent, now) {
        store::set_off_until(conn, device, until).await?;
        trail::set_off(conn, bus, device, now).await?;
    }
    Ok(())
}

/// Ends each lease that ran out by `now` and each attempt over its limit (K10 Section 7); returns
/// how many jobs changed.
///
/// # Errors
/// `Database` or `Events` when a change cannot be written; nothing is changed then.
pub async fn end_lost(queue: Queue<'_>, now: DateTime<Utc>) -> Result<usize, Error> {
    let mut tx = queue.db.begin().await?;
    let outcome = end_lost_in(&mut tx, queue.bus, now).await;
    ended(tx, outcome).await
}

async fn end_lost_in(
    conn: &mut PgConnection,
    bus: &Bus,
    now: DateTime<Utc>,
) -> Result<usize, Error> {
    let client = trail::HUB_ADDRESS;
    let mut changed = 0;
    for job in store::ended_leases(conn, now).await? {
        let Some(device) = job.holder else { continue };
        let next = lifecycle::lease_lost(&job, now);
        store::save(conn, &next).await?;
        if counts_toward_cooldown(job.asked.kind) {
            fault(conn, bus, (device, Fault::Lost), now).await?;
        }
        trail::attempt_ended(conn, &next, (device, "lease_ended"), (client, now)).await?;
        changed += 1;
    }
    for job in store::over_time(conn, now).await? {
        let Some(device) = job.holder else { continue };
        if !lifecycle::overdue(&job, now) {
            continue;
        }
        let (next, reason) = if job.asked.kind == WorkKind::Host {
            (lifecycle::orphan(&job, now), "orphan")
        } else {
            (lifecycle::expired(&job, now), "expired")
        };
        store::save(conn, &next).await?;
        trail::attempt_ended(conn, &next, (device, reason), (client, now)).await?;
        changed += 1;
    }
    Ok(changed + super::hosting::expire_waiting(conn, now).await?)
}

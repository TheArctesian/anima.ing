//! A never-answered ICMP check moves once to the first enabled port that accepts TCP (CHK-013, R-064).
//! Connects run under the target rule (R-032) before any lock; the write re-checks the locked row, so an operator's edit wins.

mod rule;
#[cfg(test)]
mod tests;

use std::net::{IpAddr, Ipv4Addr};

use chrono::{DateTime, Utc};

use super::{
    attempt::{Context, warn},
    log::Fault,
};
use crate::fleet::{
    checks::{
        Due,
        probe::{ProbeEnv, ProbeTarget, probe},
        store::repairs::{self, Seen},
    },
    devices::{self, Device},
    events::{
        self, ActorKind, AuditAction, AuditTarget, ClientAddr, NewAudit, NewEvent, Operator,
        Severity, Source,
    },
};

use rule::Port;

/// A failed read or write leaves the check untried, so a later Down beat tries again.
pub(super) async fn run(ctx: &Context, due: &Due, in_maintenance: bool, now: DateTime<Utc>) {
    // An inverted check that is Down got replies, so its method works; no read is needed.
    if due.settings.invert || in_maintenance {
        return;
    }
    let seen = match repairs::facts(&ctx.db, due.id).await {
        Ok(Some(seen)) => seen,
        Ok(None) => return,
        Err(error) => return warn(due, "the repair facts cannot be read", &error),
    };
    if !rule::is_candidate(&seen.facts, in_maintenance) {
        return;
    }
    let device = match devices::get(&ctx.db, due.device).await {
        Ok(device) => device,
        Err(error) => return warn(due, "the device of a repair cannot be read", &error),
    };
    let candidate = Candidate {
        due,
        seen: &seen,
        device: &device.name,
    };
    let written = match answering_port(ctx, &device, now).await {
        Some(port) => switch(ctx, &candidate, port, now).await,
        None => give_up(ctx, &candidate, now).await,
    };
    if let Err(error) = written {
        warn(due, "the repair cannot be written", &error);
    }
}

async fn answering_port(ctx: &Context, device: &Device, now: DateTime<Utc>) -> Option<Port> {
    let target = ProbeTarget {
        host: device.host.clone(),
    };
    let env = ProbeEnv::new(ctx.probing.policy.clone(), now);
    for port in rule::ports(device) {
        if probe(&target, &port.config(), None, &env).await.ok {
            return Some(port);
        }
    }
    None
}

/// The hub as the actor of its own repairs (D-4).
fn hub() -> (Operator, ClientAddr) {
    let operator = Operator::hub();
    (operator, ClientAddr(IpAddr::V4(Ipv4Addr::LOCALHOST)))
}

struct Candidate<'a> {
    due: &'a Due,
    seen: &'a Seen,
    device: &'a str,
}

impl Candidate<'_> {
    fn event(&self, severity: Severity, message: String) -> NewEvent {
        NewEvent {
            severity,
            source: Source::Check,
            device: Some((self.due.device, self.device.to_owned())),
            message,
            actor: ActorKind::Hub,
        }
    }
}

async fn switch(
    ctx: &Context,
    candidate: &Candidate<'_>,
    port: Port,
    now: DateTime<Utc>,
) -> Result<bool, Fault> {
    let Candidate { due, seen, device } = *candidate;
    let Some(before) = seen.facts.config.as_ref() else {
        return Ok(false);
    };
    let mut tx = ctx.db.begin().await?;
    // The device is not re-read: the next TCP beat probes its host and port as they are then.
    if !repairs::lock_unchanged(&mut tx, due.id, seen).await? {
        return Ok(false);
    }
    let taken = repairs::names_beside(&mut tx, due.device, due.id).await?;
    let name = rule::repaired_name(port, &seen.name, &taken);
    let config = port.config();
    repairs::switch_method(&mut tx, due.id, (&name, &config), now).await?;
    let (operator, client) = hub();
    let audit = NewAudit {
        operator,
        action: AuditAction::Update,
        target: AuditTarget::Check(due.id),
        client,
        change: rule::audit_change((&seen.name, before), (&name, &config)),
    };
    events::audit(&mut tx, now, audit).await?;
    let event = candidate.event(Severity::Info, rule::repaired_message(device, port));
    events::record(&mut tx, &ctx.bus, now, event).await?;
    tx.commit().await?;
    Ok(true)
}

async fn give_up(
    ctx: &Context,
    candidate: &Candidate<'_>,
    now: DateTime<Utc>,
) -> Result<bool, Fault> {
    let Candidate { due, seen, device } = *candidate;
    let mut tx = ctx.db.begin().await?;
    if !repairs::lock_unchanged(&mut tx, due.id, seen).await? {
        return Ok(false);
    }
    repairs::mark_tried(&mut tx, due.id, now).await?;
    let event = candidate.event(Severity::Warning, rule::silent_message(device));
    events::record(&mut tx, &ctx.bus, now, event).await?;
    tx.commit().await?;
    Ok(true)
}

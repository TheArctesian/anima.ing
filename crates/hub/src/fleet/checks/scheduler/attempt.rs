//! CHK-001, CHK-002, CHK-003, CHK-006. Maintenance is decided when the beat is recorded, so a
//! window that starts or ends shows on the check's next beat.

use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;

use super::{
    moved_host, repair,
    schedule::{self, Timing},
};
use crate::{
    fleet::{
        checks::{
            self, Attempt, Beat, Due, Error,
            config::CheckConfig,
            probe::{
                Failure, ProbeEnv, ProbeResult, ProbeTarget, Reach, observe_listing, probe,
                rdap::{Lookup, Rdap},
                snmp,
            },
            store,
        },
        credentials::Vault,
        devices::{self, Device},
        events::{Bus, HeartbeatSummary, Signal},
        maintenance,
        targets::AddressPolicy,
    },
    ids::{CheckId, DeviceId},
    status::Status,
};

pub struct Probing {
    pub vault: Vault,
    pub policy: AddressPolicy,
    /// `host:port` of the Tailscale `LocalAPI` proxy (D-4).
    pub localapi: Option<String>,
}

pub(super) struct Context {
    pub(super) db: PgPool,
    pub(super) bus: Bus,
    pub(super) probing: Probing,
    /// The domain expiry probes share one RDAP bootstrap file (D-29).
    pub(super) rdap: Rdap,
}

impl Context {
    /// The RDAP probes reach only what `probing` permits.
    pub(super) fn new(db: PgPool, bus: Bus, probing: Probing) -> Self {
        let rdap = Rdap::new(probing.policy.clone());
        Self {
            db,
            bus,
            probing,
            rdap,
        }
    }
}

#[derive(Debug)]
pub(super) struct Finished {
    pub(super) check: CheckId,
    pub(super) device: DeviceId,
    pub(super) recorded: Option<Timing>,
}

/// The error text holds no SQL, URL, header or secret (LOG-007).
pub(super) fn warn(due: &Due, what: &str, error: &dyn std::fmt::Display) {
    tracing::warn!(
        domain = "checks",
        check_id = %due.id,
        device_id = %due.device,
        %error,
        "{what}"
    );
}

enum Outcome {
    /// The `bool`: maintenance covered the check when the beat was recorded.
    Written(Beat, bool),
    Skipped,
    Failed {
        probed: bool,
    },
}

/// A failed read or write retries only after the retry interval, so it neither probes on every
/// tick nor starves the other checks (CHK-001). An ICMP Down beat may repair its method here,
/// so the permit covers the repair's connects (CHK-013).
pub(super) async fn run(ctx: &Context, due: Due, now: DateTime<Utc>) -> Finished {
    let mut finished = Finished {
        check: due.id,
        device: due.device,
        recorded: None,
    };
    match attempt(ctx, &due, now).await {
        Outcome::Written(beat, in_maintenance) => {
            ctx.bus.publish(Signal::Heartbeat {
                check: due.id,
                device: due.device,
                beat: HeartbeatSummary {
                    at: beat.at,
                    status: beat.status,
                    response_time_ms: beat.response_time_ms,
                    message: beat.message.clone(),
                },
            });
            finished.recorded = Some(Timing {
                next_due_at: beat.state.next_due_at,
                interval_seconds: due.settings.interval_seconds,
            });
            let icmp = matches!(due.config, Some(CheckConfig::Icmp { .. }));
            if icmp && beat.state.status == Status::Down {
                repair::run(ctx, &due, in_maintenance, now).await;
            }
        }
        Outcome::Skipped => {}
        Outcome::Failed { probed } => back_off(ctx, &due, now, probed).await,
    }
    finished
}

async fn attempt(ctx: &Context, due: &Due, now: DateTime<Utc>) -> Outcome {
    let device = match devices::get(&ctx.db, due.device).await {
        Ok(device) => device,
        Err(error) => {
            warn(due, "the device of a due check cannot be read", &error);
            return Outcome::Failed { probed: false };
        }
    };
    let Some(result) = observe(ctx, due, &device, now).await else {
        return Outcome::Failed { probed: false };
    };
    // Read for every result: the settings that decide Down are read only when the beat is written.
    let in_maintenance = match in_maintenance(&ctx.db, &device, (due.id, due.parent), now).await {
        Ok(covered) => covered,
        Err(error) => {
            warn(due, "the maintenance windows cannot be read", &error);
            return Outcome::Failed { probed: true };
        }
    };
    let attempt = Attempt {
        result: &result,
        in_maintenance,
        at: now,
        domain: match &due.config {
            Some(CheckConfig::DomainExpiry { domain, .. }) => Some(domain.as_str()),
            _ => None,
        },
        device: &device,
    };
    match record(&ctx.db, due.id, &attempt).await {
        Ok(Some(beat)) => Outcome::Written(beat, in_maintenance),
        Ok(None) => Outcome::Skipped,
        Err(Error::NotFound(_)) => {
            tracing::debug!(domain = "checks", check_id = %due.id, "the check was deleted");
            Outcome::Skipped
        }
        Err(error) => {
            warn(due, "the heartbeat cannot be recorded", &error);
            Outcome::Failed { probed: true }
        }
    }
}

/// A probe that ran counts as the last query for the check's floor, though no beat recorded
/// it (R-102).
async fn back_off(ctx: &Context, due: &Due, now: DateTime<Utc>, probed: bool) {
    let retry = TimeDelta::seconds(i64::from(due.settings.retry_interval_seconds));
    let queried_at = if probed { Some(now) } else { due.last_beat_at };
    let at = schedule::not_before_floor(due, queried_at, now + retry);
    let moved = store::set_next_due(&ctx.db, due.id, due.state.next_due_at, at).await;
    if let Err(error) = moved {
        warn(due, "the retry time cannot be stored", &error);
    }
}

/// PUSH checks are not probed: being due means no push came (CHK-002).
async fn observe(
    ctx: &Context,
    due: &Due,
    device: &Device,
    now: DateTime<Utc>,
) -> Option<ProbeResult> {
    let target = ProbeTarget {
        host: device.host.clone(),
    };
    let env =
        ProbeEnv::new(ctx.probing.policy.clone(), now).with_localapi(ctx.probing.localapi.clone());
    match &due.config {
        None => Some(ProbeResult::failed(&Failure::BadConfig)),
        Some(CheckConfig::Push) => Some(schedule::missed_push()),
        Some(CheckConfig::Http(_)) => observe_http(ctx, due, &target, &env).await,
        Some(CheckConfig::Snmp(_)) => observe_snmp(ctx, (due, device), &target, &env).await,
        Some(config @ (CheckConfig::Docker { .. } | CheckConfig::Systemd { .. })) => {
            let reach = Reach {
                db: &ctx.db,
                vault: &ctx.probing.vault,
                policy: &ctx.probing.policy,
            };
            Some(observe_listing(&reach, device, config, now).await)
        }
        Some(
            config @ CheckConfig::DomainExpiry {
                domain, warn_days, ..
            },
        ) => {
            let lookup = Lookup {
                domain,
                warn_days: *warn_days,
                timeout: config.timeout(),
            };
            Some(ctx.rdap.observe(lookup, now).await)
        }
        Some(config) => Some(probe(&target, config, None, &env).await),
    }
}

/// Probes the configuration stored now with its secrets, never the one read at tick time: the
/// secrets go only where they were given for (SEC-005).
async fn observe_http(
    ctx: &Context,
    due: &Due,
    target: &ProbeTarget,
    env: &ProbeEnv,
) -> Option<ProbeResult> {
    let setup = match checks::load_http_secrets(&ctx.db, &ctx.probing.vault, due.id).await {
        Ok(Some(setup)) => setup,
        Ok(None) => return Some(ProbeResult::failed(&Failure::BadConfig)),
        Err(error) => return unopened(due, &error),
    };
    let config = CheckConfig::Http(setup.config);
    Some(probe(target, &config, setup.secrets.as_ref(), env).await)
}

fn unopened(due: &Due, error: &Error) -> Option<ProbeResult> {
    match error {
        Error::Unreadable(_) => Some(ProbeResult::failed(&Failure::BadConfig)),
        Error::Credentials(_) => Some(ProbeResult::failed(&Failure::Secrets)),
        Error::SecretsUnbound => Some(ProbeResult::failed(&Failure::SecretsUnbound)),
        error => {
            warn(due, "the check secrets cannot be read", error);
            None
        }
    }
}

/// The secrets go only to the version, user name and device host they were given for (SEC-005,
/// SEC-010); secrets for another host are removed and nothing is sent.
async fn observe_snmp(
    ctx: &Context,
    (due, device): (&Due, &Device),
    target: &ProbeTarget,
    env: &ProbeEnv,
) -> Option<ProbeResult> {
    let host = target.host.as_str();
    match checks::load_snmp_secrets(&ctx.db, &ctx.probing.vault, due.id, host).await {
        Ok(Some(checks::SnmpSetup {
            config,
            secrets: Some(secrets),
        })) => {
            let draws = snmp::Draws::fresh()
                .inspect_err(|error| warn(due, "the SNMP request ids cannot be drawn", error))
                .ok()?;
            Some(snmp::observe(target, (&config, &secrets), env, draws).await)
        }
        Ok(Some(_)) => Some(ProbeResult::failed(&Failure::SecretMissing)),
        Ok(None) => Some(ProbeResult::failed(&Failure::BadConfig)),
        Err(Error::SecretsForOtherHost) => {
            moved_host::forget_secrets(ctx, (due, device), host, env.now).await;
            Some(ProbeResult::failed(&Failure::SecretsForOtherHost))
        }
        Err(error) => unopened(due, &error),
    }
}

/// The scheduler and the push route both ask this, so every beat is recorded under the same
/// cover (CHK-006).
pub(in crate::fleet::checks) async fn in_maintenance(
    db: &PgPool,
    device: &Device,
    (check, parent): (CheckId, Option<CheckId>),
    now: DateTime<Utc>,
) -> Result<bool, maintenance::Error> {
    if device.maintenance {
        return Ok(true);
    }
    let active = maintenance::active_targets(db, now).await?;
    Ok([Some(check), parent]
        .into_iter()
        .flatten()
        .any(|check| active.covers(device.id, Some(check), &device.tags)))
}

async fn record(db: &PgPool, check: CheckId, attempt: &Attempt<'_>) -> Result<Option<Beat>, Error> {
    let mut tx = db.begin().await?;
    let beat = checks::record_attempt(&mut tx, check, attempt).await?;
    tx.commit().await?;
    Ok(beat)
}

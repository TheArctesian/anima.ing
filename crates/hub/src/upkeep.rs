//! Starts the hub's background work; each job only calls its domains (R-072, D-4, DB-004).

mod hub_device;
mod link_presence;

use std::{sync::Arc, time::Duration};

use anima_hub::{
    clock::Clock,
    config::Config,
    engine::{locations, reminders, sources},
    fleet::{
        access, agent_threads, builds, checks,
        credentials::{Vault, operational},
        events::{self, Bus},
        metrics, notifications,
        retention::{self, Subject},
        targets::AddressPolicy,
        vpn,
    },
    schema::Dependencies,
    work::{checkout, jobs},
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tokio::time::MissedTickBehavior;

/// The checks that a hosted hub reports itself (HMD-045, HMD-047).
pub struct Hosted {
    alive: Option<hub_device::Alive>,
    watch: Option<link_presence::Watch>,
}

/// Makes the hub's own checks ready, before the scheduler can find one due.
pub async fn hosted(config: &Config, db: &PgPool, bus: &Bus, clock: &Arc<dyn Clock>) -> Hosted {
    Hosted {
        alive: hub_device::start(config, db, bus, clock).await,
        watch: link_presence::start(config, db, bus, clock).await,
    }
}

impl Hosted {
    fn spawn(self) -> Vec<tokio::task::JoinHandle<()>> {
        let alive = self.alive.map(|alive| tokio::spawn(alive.keep_reporting()));
        let watch = self.watch.map(|watch| tokio::spawn(watch.keep()));
        alive.into_iter().chain(watch).collect()
    }
}

/// Starts the scheduled jobs (D-4, DB-004, DB-005, D-25, BLD-025, LOG-028; WP-36 D-2, D-25).
pub fn spawn(dependencies: &Dependencies, hosted: Hosted) -> Vec<tokio::task::JoinHandle<()>> {
    let (pool, clock) = (&dependencies.pool, &dependencies.clock);
    let retention = {
        let db = pool.clone();
        events::run_daily("retention", clock.clone(), move |now| {
            let db = db.clone();
            async move { keep(&db, now).await }
        })
    };
    let prune = {
        let db = pool.clone();
        events::run_daily("access_prune", clock.clone(), move |now| {
            let db = db.clone();
            async move {
                let builders = builds::builders(&db).await;
                let before = builders.map_err(|e| e.fault().unwrap_or("builds"))?;
                access::prune(&db, now)
                    .await
                    .map_err(|e| e.fault().unwrap_or("access"))?;
                builds::audit_removed(&db, &before, now, builds::Removal::TokenExpired)
                    .await
                    .map(drop)
                    .map_err(|e| e.fault().unwrap_or("builds"))
            }
        })
    };
    let chain = {
        let (db, bus) = (pool.clone(), dependencies.bus.clone());
        events::run_daily("audit_chain", clock.clone(), move |now| {
            let (db, bus) = (db.clone(), bus.clone());
            async move {
                events::check_chain_daily(&db, &bus, now)
                    .await
                    .map_err(|e| e.fault())
            }
        })
    };
    let builds = Builds {
        db: pool.clone(),
        bus: dependencies.bus.clone(),
        updates: dependencies.build_updates.clone(),
        clock: clock.clone(),
    };
    let mut jobs = vec![
        tokio::spawn(retention),
        tokio::spawn(prune),
        tokio::spawn(chain),
    ];
    jobs.push(tokio::spawn(publishing(
        builds.clone(),
        dependencies.bucket.clone(),
    )));
    jobs.push(tokio::spawn(job_leases(
        pool.clone(),
        dependencies.bus.clone(),
        clock.clone(),
    )));
    jobs.push(tokio::spawn(build_leases(builds)));
    jobs.extend(loops(dependencies));
    jobs.extend(hosted.spawn());
    jobs
}

/// Starts the loops that watch the devices and send (R-072, SRC-021, VPN-008).
fn loops(dependencies: &Dependencies) -> Vec<tokio::task::JoinHandle<()>> {
    let (pool, bus, clock) = (&dependencies.pool, &dependencies.bus, &dependencies.clock);
    let source_schedule = sources::Scheduling::new(
        (pool.clone(), bus.clone(), clock.clone()),
        dependencies.vault.clone(),
        dependencies.sources.clone(),
    );
    let (probing, collecting, sending) = reaching(
        &dependencies.vault,
        &dependencies.policy,
        dependencies.config.tailscale_localapi.clone(),
    );
    let mut loops = vec![
        tokio::spawn(remote_receipts(pool.clone())),
        checks::spawn_scheduler(pool.clone(), bus.clone(), clock.clone(), probing),
        metrics::spawn_collector(
            pool.clone(),
            dependencies.metrics_feed.clone(),
            bus.clone(),
            clock.clone(),
            collecting,
        ),
        notifications::spawn_dispatcher(pool.clone(), bus.clone(), clock.clone(), sending),
        reminders::spawn_minute(
            reminders::Sending {
                db: pool.clone(),
                bus: bus.clone(),
                vault: dependencies.vault.clone(),
                live: dependencies.reminders.clone(),
            },
            reminders::Reading {
                seal: locations::Seal::new(dependencies.files.sealing_keys().clone()),
                extensions: Vec::new(),
                sources: dependencies.sources.registry.clone(),
            },
            clock.clone(),
        ),
        metrics::spawn_stale_watch(pool.clone(), bus.clone(), clock.clone()),
        sources::spawn_scheduler(source_schedule.clone()),
        checkout::spawn(source_schedule),
    ];
    let reading = vpn::Reading {
        pool: pool.clone(),
        bus: bus.clone(),
        clock: clock.clone(),
        feed: dependencies.vpn_feed.clone(),
        broker: dependencies.vpn_broker.clone(),
    };
    loops.extend(vpn::spawn_reader(
        dependencies.config.tailscale_localapi.clone(),
        reading,
    ));
    loops
}

/// Checks, metrics and notifications reach the outside through the one address policy (P2-004,
/// D-16).
fn reaching(
    vault: &Vault,
    policy: &AddressPolicy,
    localapi: Option<String>,
) -> (checks::Probing, metrics::Collecting, notifications::Sending) {
    let (vault, policy) = (vault.clone(), policy.clone());
    (
        checks::Probing {
            vault: vault.clone(),
            policy: policy.clone(),
            localapi,
        },
        metrics::Collecting {
            vault: vault.clone(),
            policy: policy.clone(),
        },
        notifications::Sending { vault, policy },
    )
}

#[derive(Clone)]
struct Builds {
    db: PgPool,
    bus: Bus,
    updates: builds::Updates,
    clock: Arc<dyn Clock>,
}

impl Builds {
    const fn queue(&self) -> builds::BuildQueue<'_> {
        builds::BuildQueue {
            db: &self.db,
            bus: &self.bus,
            updates: &self.updates,
        }
    }
}

/// A failed read of the periods skips the run, so a period is never guessed.
async fn keep(db: &PgPool, now: DateTime<Utc>) -> Result<(), &'static str> {
    let periods = retention::periods(db).await.map_err(|e| e.fault())?;
    let checks = checks::drop_expired(db, now, periods.keep(Subject::Heartbeats))
        .await
        .map_err(|e| e.fault());
    let event_logs = events::keep_partitions(db, now, periods.keep(Subject::Events))
        .await
        .map_err(|e| e.fault());
    let samples = metrics::keep_partitions(db, now, periods.keep(Subject::Metrics))
        .await
        .map_err(|e| e.fault());
    let deliveries = notifications::keep_deliveries(db, now, periods.keep(Subject::Deliveries))
        .await
        .map_err(|e| e.fault());
    // The sealed value of an ended operational key version goes after 30 days (R-147).
    let ended_keys = operational::forget_ended(db, now)
        .await
        .map(drop)
        .map_err(|_| "credentials");
    // Each ended hour of an ingest token gets its audit entry, even with no later fix (R-262).
    let fix_hours = locations::audit_ended_hours(db, now)
        .await
        .map(drop)
        .map_err(|_| "locations");
    checks
        .and(event_logs)
        .and(samples)
        .and(deliveries)
        .and(ended_keys)
        .and(fix_hours)
}

/// Ends the build leases that ran out, so the Builds view is true without a claim (D-2).
async fn build_leases(builds: Builds) {
    let mut ticks = tokio::time::interval(builds::LEASE_SWEEP);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        if let Err(error) = builds::expire_leases(builds.queue(), builds.clock.now()).await {
            let fault = error.fault().unwrap_or("builds");
            tracing::warn!(job = "build_leases", fault, "scheduled job failed");
        }
    }
}

/// Ends lapsed job leases and attempts over their limit (K10 Section 7).
async fn job_leases(db: PgPool, bus: Bus, clock: Arc<dyn Clock>) {
    let mut ticks = tokio::time::interval(jobs::LEASE_SWEEP);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        let queue = jobs::Queue { db: &db, bus: &bus };
        if jobs::end_lost(queue, clock.now()).await.is_err() {
            tracing::warn!(job = "job_leases", fault = "jobs", "scheduled job failed");
        }
    }
}

/// Closes expired remote receipts and publication deadlines (K10 Section 6.5).
async fn remote_receipts(db: PgPool) {
    let mut ticks = tokio::time::interval(jobs::LEASE_SWEEP);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        if agent_threads::sweep_remote(&db).await.is_err() {
            tracing::warn!(
                job = "remote_receipts",
                fault = "agent_threads",
                "scheduled job failed"
            );
        }
    }
}

/// Publishes the build jobs whose files are in (WP-36 D-25).
async fn publishing(builds: Builds, bucket: Option<Arc<builds::Bucket>>) {
    let Ok(http) = publishing_client() else {
        job_failed("build_publishing", "http client");
        return;
    };
    let mut ticks = tokio::time::interval(builds::PUBLISH_SWEEP);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        let swept = match &bucket {
            Some(bucket) => {
                builds::publish_ready(builds.queue(), bucket, &http, builds.clock.as_ref()).await
            }
            None => builds::publishing_off(builds.queue(), builds.clock.now()).await,
        };
        if let Err(error) = swept {
            job_failed("build_publishing", error.fault().unwrap_or("builds"));
        }
    }
}

/// Logs a failed run by its fault word only (LOG-007).
fn job_failed(job: &'static str, fault: &'static str) {
    tracing::warn!(job, fault, "scheduled job failed");
}

/// No redirect, so the public check is of the public address itself (BLD-064).
pub(crate) fn publishing_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(60))
        .build()
}

#[cfg(test)]
mod tests;

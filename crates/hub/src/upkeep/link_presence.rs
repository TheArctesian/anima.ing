//! Keeps the check `Connected` of each computer on a hosted hub's link (HMD-047, R-143).

use std::{collections::HashSet, net::IpAddr, path::PathBuf, sync::Arc, time::Duration};

use anima_hub::{
    clock::Clock,
    config::Config,
    fleet::{
        checks::{self, OwnCheck, OwnReport},
        devices::{self, Device},
        events::Bus,
        link::{self, Presence},
    },
    ids::{CheckId, DeviceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;
use tokio::time::MissedTickBehavior;

use super::job_failed;

const JOB: &str = "link_presence";
/// A computer signs that it is connected each 15 seconds.
const EVERY: Duration = Duration::from_secs(15);
/// Three of a computer's signs.
const FRESH: TimeDelta = TimeDelta::seconds(45);
/// A longer wait between two passes means that the hub slept.
const ASLEEP: TimeDelta = TimeDelta::seconds(45);
const GONE: &str = "Not connected";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Sign {
    Here,
    Gone,
    /// The link never saw the computer, or the hub did not watch long enough to know.
    Unknown,
}

pub(super) fn sign(
    seen: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    watching_since: DateTime<Utc>,
) -> Sign {
    match seen {
        Some(at) if now - at <= FRESH => Sign::Here,
        Some(_) if now - watching_since >= FRESH => Sign::Gone,
        _ => Sign::Unknown,
    }
}

pub(super) fn computers(
    devices: &[Device],
    presence: &[Presence],
) -> Vec<(DeviceId, Option<DateTime<Utc>>)> {
    let on_link = |device: &Device| {
        let host: IpAddr = device.host.parse().ok()?;
        let listed = presence.iter().find(|listed| listed.address == host)?;
        Some((device.id, listed.seen))
    };
    devices.iter().filter_map(on_link).collect()
}

pub struct Watch {
    control: PathBuf,
    db: PgPool,
    bus: Bus,
    clock: Arc<dyn Clock>,
    /// Its start, or the end of its last sleep.
    since: DateTime<Utc>,
    last: DateTime<Utc>,
    /// The checks reported Down since their computer was last connected.
    gone: HashSet<CheckId>,
}

/// Makes each check `Connected` wait one whole interval, before the scheduler finds it due.
pub async fn start(
    config: &Config,
    db: &PgPool,
    bus: &Bus,
    clock: &Arc<dyn Clock>,
) -> Option<Watch> {
    config.hosted_cloud_key_file.as_ref()?;
    let control = config.link_control.clone()?;
    let mut watch = Watch::new(control, (db.clone(), bus.clone()), clock.clone());
    watch.begin(clock.now()).await;
    Some(watch)
}

impl Watch {
    pub(super) fn new(control: PathBuf, (db, bus): (PgPool, Bus), clock: Arc<dyn Clock>) -> Self {
        let now = clock.now();
        Self {
            control,
            db,
            bus,
            clock,
            since: now,
            last: now,
            gone: HashSet::new(),
        }
    }

    pub(super) async fn begin(&mut self, now: DateTime<Utc>) {
        self.since = now;
        if let Err(error) = checks::own::await_reports(&self.db, OwnCheck::Connected, now).await {
            job_failed(JOB, error.fault());
        }
    }

    pub async fn keep(mut self) {
        let mut ticks = tokio::time::interval(EVERY);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            let now = self.clock.now();
            if now - self.last > ASLEEP {
                self.begin(now).await;
            }
            self.last = now;
            let reported = match link::presence(&self.control).await {
                Ok(presence) => self.pass(&presence, now).await,
                Err(fault) => Err(fault),
            };
            if let Err(fault) = reported {
                job_failed(JOB, fault);
            }
        }
    }

    pub(super) async fn pass(
        &mut self,
        presence: &[Presence],
        now: DateTime<Utc>,
    ) -> Result<(), &'static str> {
        let listed = devices::list(&self.db).await.map_err(|_| "devices")?;
        for (device, seen) in computers(&listed, presence) {
            let reported = match sign(seen, now, self.since) {
                Sign::Here => self.here(device, now).await,
                Sign::Gone => self.gone(device, now).await,
                Sign::Unknown => Ok(()),
            };
            reported.map_err(|error| error.fault())?;
        }
        Ok(())
    }

    async fn here(&mut self, device: DeviceId, now: DateTime<Utc>) -> Result<(), checks::Error> {
        let mut conn = self.db.acquire().await?;
        let check = checks::own::ensure(&mut conn, device, OwnCheck::Connected).await?;
        drop(conn);
        self.gone.remove(&check);
        checks::own::report((&self.db, &self.bus), check, OwnReport::Up, now).await?;
        Ok(())
    }

    async fn gone(&mut self, device: DeviceId, now: DateTime<Utc>) -> Result<(), checks::Error> {
        let Some(check) = checks::own::find(&self.db, device, OwnCheck::Connected).await? else {
            return Ok(());
        };
        if self.gone.insert(check) {
            let down = OwnReport::Down(GONE);
            checks::own::report((&self.db, &self.bus), check, down, now).await?;
        }
        Ok(())
    }
}

//! Reads the hub's VPN status each minute, publishes the picture and writes each new warning
//! once (VPN-008, VPN-010, VPN-011).

use std::{collections::HashSet, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use ipnet::IpNet;
use sqlx::PgPool;
use tokio::time::MissedTickBehavior;

use super::{
    Error,
    broker::{Broker, BrokerView},
    feed::Feed,
    localapi,
    model::{BrokerState, Picture},
    peers::Status,
    picture::{self, Inputs},
    store,
    warnings::{self, Warning},
};
use crate::{
    clock::Clock,
    fleet::{
        checks::{self, config::CheckType},
        devices,
        events::{self, ActorKind, Bus, NewEvent, Source},
    },
    ids::{DeviceId, SiteId},
};

const INTERVAL: Duration = Duration::from_secs(60);
/// As long as the `LocalAPI` read may take (BD-15).
pub(super) const BROKER_WAIT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct Reading {
    pub pool: PgPool,
    pub bus: Bus,
    pub clock: Arc<dyn Clock>,
    pub feed: Feed,
    /// The hub's one broker, read on each read (BD-16); None when `ANIMA_VPN_BROKER` is unset.
    pub broker: Option<Broker>,
}

pub(super) type Brokered = Option<Result<BrokerView, Error>>;

/// `None` when the `LocalAPI` proxy is not set; the vpn query then answers null (VPN-008).
#[must_use]
pub fn spawn_reader(
    localapi: Option<String>,
    reading: Reading,
) -> Option<tokio::task::JoinHandle<()>> {
    let localapi = localapi?;
    Some(tokio::spawn(async move {
        let mut ticks = tokio::time::interval(INTERVAL);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut failed = false;
        loop {
            ticks.tick().await;
            read_once(&localapi, &reading, &mut failed).await;
        }
    }))
}

pub(super) async fn read_once(localapi: &str, reading: &Reading, last_failed: &mut bool) {
    let outcome = read_and_publish(localapi, reading).await;
    log_change(outcome.as_ref().err(), *last_failed);
    *last_failed = outcome.is_err();
}

/// Only a change, by the fault word only (LOG-007): an outage writes two lines, not one a minute.
fn log_change(failure: Option<&Error>, last_failed: bool) {
    match (failure, last_failed) {
        (None, true) => read_again(),
        (Some(error), false) => read_failed(error.fault()),
        _ => {}
    }
}

fn read_failed(fault: &'static str) {
    tracing::warn!(job = "vpn_reader", fault, "VPN status read failed");
}

fn read_again() {
    tracing::info!(job = "vpn_reader", "VPN status read again");
}

/// Logs only a change of the broker's state, by the fault word only (LOG-007).
fn log_broker_change(previous: Option<BrokerState>, state: BrokerState, brokered: &Brokered) {
    if previous == Some(state) {
        return;
    }
    match brokered {
        Some(Err(error)) => broker_failed(error.fault()),
        Some(Ok(_)) if previous.is_some() => broker_again(),
        _ => {}
    }
}

fn broker_failed(fault: &'static str) {
    tracing::warn!(job = "vpn_reader", fault, "VPN broker read failed");
}

fn broker_again() {
    tracing::info!(job = "vpn_reader", "VPN broker read again");
}

pub(super) fn broker_answer<'a>(
    brokered: &'a Brokered,
    last: Option<&'a Picture>,
) -> (BrokerState, Option<&'a BrokerView>) {
    match (brokered, last) {
        (None, _) => (BrokerState::NotSet, None),
        (Some(Ok(view)), _) => (BrokerState::Ready, Some(view)),
        (Some(Err(Error::BrokerBusy)), Some(last)) => (last.broker, last.roster.as_ref()),
        (Some(Err(Error::NotInstalled)), _) => (BrokerState::NotInstalled, None),
        (Some(Err(_)), _) => (BrokerState::Failed, None),
    }
}

/// The task holds the broker's turn to its end, which comes by itself within the call limit.
pub(super) async fn read_broker(broker: Option<&Broker>, wait: Duration) -> Brokered {
    let broker = broker?.clone();
    let mut task = tokio::spawn(async move { broker.nodes_if_free().await });
    match tokio::time::timeout(wait, &mut task).await {
        Ok(Ok(answer)) => Some(answer),
        Ok(Err(failed)) => std::panic::resume_unwind(failed.into_panic()),
        Err(_) => Some(Err(Error::BrokerBusy)),
    }
}

/// A failed warning write keeps the published picture; its marks are not written, so the next
/// read tries again.
async fn read_and_publish(localapi: &str, reading: &Reading) -> Result<(), Error> {
    let now = reading.clock.now();
    let (status, brokered) = tokio::join!(
        localapi::status(localapi),
        read_broker(reading.broker.as_ref(), BROKER_WAIT)
    );
    let previous = reading.feed.latest();
    let answer = broker_answer(&brokered, previous.as_deref());
    let built = match status {
        Ok(status) => build_picture(&status, answer, &reading.pool, now).await,
        Err(error) => Err(error),
    };
    let built = match built {
        Ok(built) => built,
        Err(error) => {
            keep_stale(&reading.feed);
            return Err(error);
        }
    };
    log_broker_change(
        previous.as_ref().map(|latest| latest.broker),
        answer.0,
        &brokered,
    );
    let picture = match previous {
        Some(previous) => picture::carry(&previous, built),
        None => built,
    };
    let due = warnings::due(&picture, now);
    reading.feed.publish(picture);
    write_warnings(&reading.pool, &reading.bus, &due, now).await
}

/// A picture already not fresh is left as it is, so subscribers are not woken for nothing.
fn keep_stale(feed: &Feed) {
    if let Some(latest) = feed.latest().filter(|latest| latest.fresh) {
        feed.publish(Picture {
            fresh: false,
            ..Picture::clone(&latest)
        });
    }
}

async fn build_picture(
    status: &Status,
    (broker_state, broker): (BrokerState, Option<&BrokerView>),
    pool: &PgPool,
    now: DateTime<Utc>,
) -> Result<Picture, Error> {
    let devices = devices::list(pool).await?;
    let sites: Vec<(SiteId, IpNet)> = devices::sites(pool)
        .await?
        .into_iter()
        .map(|site| (site.id, site.subnet))
        .collect();
    let routers = picture::router_devices(status, &sites, &devices);
    let ping_checked: HashSet<DeviceId> = checks::for_devices(pool, &routers)
        .await?
        .into_values()
        .flatten()
        .filter(|check| check.config.kind() == CheckType::TailscalePing && !check.paused)
        .map(|check| check.device)
        .collect();
    picture::build(&Inputs {
        status,
        sites: &sites,
        devices: &devices,
        ping_checked: &ping_checked,
        now,
        broker_state,
        broker,
    })
}

pub(super) async fn write_warnings(
    pool: &PgPool,
    bus: &Bus,
    warnings: &[Warning],
    now: DateTime<Utc>,
) -> Result<(), Error> {
    let mut tx = pool.begin().await?;
    for warning in warnings {
        if store::mark(&mut tx, warning, now).await? {
            let event = NewEvent {
                severity: warning.severity,
                source: Source::Vpn,
                device: warning.device.clone(),
                message: warning.message.clone(),
                actor: ActorKind::Hub,
            };
            events::record(&mut tx, bus, now, event).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

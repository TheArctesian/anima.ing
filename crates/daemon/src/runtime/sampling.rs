//! Inventory probing and telemetry sampling have independent clocks and task lifetimes.
use crate::{
    push::Event,
    readers::{self, Platform, Source, container::Container, gpu},
    sampling::{
        Sampler,
        batteries::BatteryWatch,
        drives::DriveWatch,
        gpu::Discovery,
        inventory::{InventoryWatch, Watch as Announced},
    },
};
use chrono::Utc;
use daemon_core::inventory::Inventory;
use std::{path::PathBuf, time::Duration};
use tokio::{
    sync::mpsc,
    time::{Instant, MissedTickBehavior, interval, interval_at},
};

/// The time between samplings (MET-014).
const SAMPLE_EVERY: Duration = Duration::from_secs(2);

/// The time between two probe runs of the tool inventory (CAP-027).
const PROBE_EVERY: Duration = Duration::from_mins(15);
const ACCESS_PROBE_EVERY: Duration = Duration::from_secs(30);

pub(super) fn source(hosted_hub: Option<PathBuf>) -> Source {
    hosted_hub.map_or(Source::Machine(Platform::current()), |data| {
        let processors = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        let processors = u64::try_from(processors).unwrap_or(1);
        Source::HostedHub(Container::new(PathBuf::from("/"), data, processors))
    })
}

pub(super) async fn sample(mut source: Source, events: mpsc::Sender<Event>) {
    let mac = matches!(source, Source::Machine(Platform::Macos));
    let gpus = source.has_gpus();
    let mut sampler = Sampler::new();
    let mut drives = DriveWatch::new();
    let mut batteries = BatteryWatch::new();
    let mut watch = Watch::start(discover(mac, gpus, &events).await, Instant::now());
    let mut ticks = interval(SAMPLE_EVERY);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticks.tick().await;
        if watch.due(Instant::now()) {
            watch.replace(discover(mac, gpus, &events).await, Instant::now());
        }
        let found = &watch.found;
        let now = Utc::now();
        let reading = source.reading(now, std::time::Instant::now()).await;
        if let Some(changed) = drives.changed(reading.drives.as_deref())
            && events.send(Event::Drives(changed)).await.is_err()
        {
            return;
        }
        if let Some(changed) = batteries.changed(reading.batteries.as_deref())
            && events.send(Event::Batteries(changed)).await.is_err()
        {
            return;
        }
        let gpus = found
            .placement
            .place_all(gpu::read(found.sources, mac).await);
        if let Some(point) = sampler.take(now, reading, &gpus)
            && events.send(Event::Point(point)).await.is_err()
        {
            return;
        }
    }
}

pub(super) async fn probe(platform: Platform, events: mpsc::Sender<Event>) {
    let mut watch = InventoryWatch::new();
    let mut hardware = Announced::new();
    let mut ticks = interval(PROBE_EVERY);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut access = interval_at(Instant::now() + ACCESS_PROBE_EVERY, ACCESS_PROBE_EVERY);
    access.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last: Option<Inventory> = None;
    loop {
        let full = tokio::select! {
            _ = ticks.tick() => true,
            _ = access.tick(), if last.as_ref().is_some_and(|read| read.remote_access.is_some()) => false,
        };
        let read = if full {
            let read = readers::inventory(platform).await;
            if let Some(found) = &read {
                last = Some(found.clone());
            }
            read
        } else {
            if let Some(found) = &mut last {
                readers::refresh_remote_access(found).await;
            }
            last.clone()
        };
        if let Some(changed) = watch.changed(read, std::time::Instant::now())
            && events.send(Event::Inventory(changed)).await.is_err()
        {
            return;
        }
        if full
            && let Some(changed) =
                hardware.changed(readers::hardware(platform).await, std::time::Instant::now())
            && events.send(Event::Hardware(changed.clone())).await.is_err()
        {
            return;
        }
    }
}

struct Watch {
    found: Discovery,
    next: Instant,
}

impl Watch {
    fn start(found: Discovery, now: Instant) -> Self {
        let next = found.next_at(now, None);
        Self { found, next }
    }

    fn due(&self, now: Instant) -> bool {
        now >= self.next
    }

    fn replace(&mut self, again: Discovery, now: Instant) {
        self.next = again.next_at(now, Some(&self.found));
        self.found = again;
    }
}

/// Tells the pusher the whole set, `[]` included (K8 Section 4).
async fn discover(mac: bool, look: bool, events: &mpsc::Sender<Event>) -> Discovery {
    let read = if look {
        gpu::read(gpu::Sources::ALL, mac).await
    } else {
        Vec::new()
    };
    let found = Discovery::new(read);
    events.send(Event::Gpus(found.announce.clone())).await.ok();
    found
}

#[cfg(all(test, target_os = "linux"))]
mod tests;

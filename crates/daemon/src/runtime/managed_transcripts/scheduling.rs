//! Four independent lanes prevent one slow node exchange from stalling every transcript.
use super::{HubTarget, Registry, publish};
use std::{collections::BTreeMap, time::Duration};
use tokio::time::Instant;
use uuid::Uuid;

pub(super) async fn worker(registry: &Registry, target: &HubTarget, lane: u8) {
    let mut due = BTreeMap::<Uuid, Instant>::new();
    loop {
        let now = Instant::now();
        let Some(ids) = registry.ids() else {
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        };
        due.retain(|id, _| ids.contains(id));
        for id in ids.into_iter().filter(|id| id.as_bytes()[15] % 4 == lane) {
            due.entry(id).or_insert(now);
        }
        let next = due
            .iter()
            .filter(|(_, at)| **at <= now)
            .min_by_key(|(_, at)| **at)
            .map(|(id, _)| *id);
        let Some(id) = next else {
            tokio::time::sleep(Duration::from_millis(250)).await;
            continue;
        };
        let delay = if let Some(journal) = registry.read(id) {
            publish(registry, target, journal).await
        } else {
            Duration::from_secs(5)
        };
        due.insert(id, Instant::now() + delay);
    }
}

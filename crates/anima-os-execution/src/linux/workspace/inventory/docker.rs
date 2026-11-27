//! Match a running container to the exact workspace receipt before observing usage.
use crate::linux::{command, control::inventory::usage};
use serde::Deserialize;
use std::{collections::BTreeMap, time::Duration};
use uuid::Uuid;

use super::record::Record;

const FORMAT: &str = concat!(
    r#"{"id":{{json .ID}},"name":{{json .Names}},"state":{{json .State}},"#,
    r#""workspace":{{json (.Label "ing.anima.workspace")}},"run":{{json (.Label "ing.anima.run")}},"#,
    r#""generation":{{json (.Label "ing.anima.generation")}}}"#,
);

#[derive(Deserialize)]
struct Container {
    id: String,
    name: String,
    state: String,
    workspace: Uuid,
    run: Uuid,
    generation: String,
}

#[derive(Default)]
pub(super) struct Running(BTreeMap<String, Option<Container>>);

impl Running {
    pub(super) async fn observe() -> Self {
        let result = tokio::time::timeout(
            Duration::from_secs(3),
            command::docker(&[
                "ps",
                "--no-trunc",
                "--filter",
                "label=ing.anima.workspace",
                "--format",
                FORMAT,
            ]),
        )
        .await;
        let mut containers = BTreeMap::new();
        if let Ok(Ok(output)) = result {
            for row in output
                .lines()
                .filter_map(|line| serde_json::from_str::<Container>(line).ok())
            {
                containers
                    .entry(row.name.clone())
                    .and_modify(|entry| *entry = None)
                    .or_insert(Some(row));
            }
        }
        Self(containers)
    }

    pub(super) fn usage(&self, record: &Record) -> Option<usage::Usage> {
        let container = self.0.get(&record.container())?.as_ref()?;
        if !matches!(container.state.as_str(), "running" | "paused")
            || container.workspace != record.workspace
            || Some(container.run) != record.run
            || container.generation != record.generation.to_string()
            || container.id.len() != 64
            || !container.id.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return None;
        }
        usage::read(&format!("docker-{}.scope", container.id))
    }
}

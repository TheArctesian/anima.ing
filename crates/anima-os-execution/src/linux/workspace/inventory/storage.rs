//! Read the Btrfs accounting for the same subvolume recorded as a workspace home.
use super::record::{self, Record};
use crate::linux::command;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

const PERSIST: &str = "/persist";

#[derive(Deserialize)]
struct MountTable {
    filesystems: [Mount; 1],
}

#[derive(Deserialize)]
struct Mount {
    fstype: String,
    uuid: Uuid,
    fsroot: PathBuf,
}

#[derive(Deserialize)]
struct QuotaTable {
    #[serde(rename = "qgroup-show")]
    groups: Vec<QuotaGroup>,
}

#[derive(Deserialize)]
struct QuotaGroup {
    qgroupid: String,
    path: PathBuf,
    referenced: u64,
    max_referenced: serde_json::Value,
}

pub(super) struct Accounting {
    root: PathBuf,
    groups: HashMap<PathBuf, Option<QuotaGroup>>,
}

impl Accounting {
    pub(super) async fn observe() -> Option<Self> {
        tokio::time::timeout(Duration::from_secs(3), Self::read())
            .await
            .ok()?
    }

    async fn read() -> Option<Self> {
        let mounts = command::run(
            "findmnt",
            &[
                "--json",
                "--mountpoint",
                PERSIST,
                "--output",
                "FSTYPE,UUID,FSROOT",
            ],
        )
        .await
        .ok()?;
        let MountTable {
            filesystems: [mount],
        } = serde_json::from_str(&mounts).ok()?;
        let root = mount.fsroot.strip_prefix("/").ok()?.to_owned();
        if mount.fstype != "btrfs" || !mount.consistent() {
            return None;
        }
        let output = command::run("btrfs", &["--format", "json", "qgroup", "show", PERSIST])
            .await
            .ok()?;
        if !mount.consistent() {
            return None;
        }
        Self::from_output(root, &output)
    }

    fn from_output(root: PathBuf, output: &str) -> Option<Self> {
        let QuotaTable { groups } = serde_json::from_str(output).ok()?;
        let mut by_path = HashMap::new();
        for group in groups {
            if group.qgroupid.starts_with("0/") && group.max_referenced.as_u64().is_some() {
                by_path
                    .entry(group.path.clone())
                    .and_modify(|entry| *entry = None)
                    .or_insert(Some(group));
            }
        }
        Some(Self {
            root,
            groups: by_path,
        })
    }

    pub(super) fn referenced_bytes(&self, receipt: &Record) -> Option<u64> {
        let home = record::home(receipt.workspace);
        let path = self.root.join(home.strip_prefix(PERSIST).ok()?);
        let quota = self.groups.get(&path)?.as_ref()?;
        (quota.max_referenced.as_u64()? == receipt.storage_bytes).then_some(quota.referenced)
    }
}

impl Mount {
    fn consistent(&self) -> bool {
        let path = format!("/sys/fs/btrfs/{}/qgroups/inconsistent", self.uuid);
        fs::read_to_string(Path::new(&path)).is_ok_and(|state| state.trim() == "0")
    }
}

#[cfg(test)]
mod tests;

//! Reader of `lsblk -J -b`: the drives of a Linux device and their mounts (MET-002, MET-027).

use serde::Deserialize;

use super::label;
use crate::points::{MAX_LABELS, is_operator_mount};

/// For util-linux 2.37 and later, which has `MOUNTPOINTS`.
pub const LSBLK_ARGS: [&str; 4] = ["-J", "-b", "-o", "NAME,TYPE,SIZE,ROTA,TRAN,MOUNTPOINTS"];

/// For an older util-linux, which refuses `MOUNTPOINTS`.
pub const LSBLK_OLD_ARGS: [&str; 4] = ["-J", "-b", "-o", "NAME,TYPE,SIZE,ROTA,TRAN,MOUNTPOINT"];

/// Partition, crypt, LVM and RAID stack a few levels at most; the bound stops a hostile document.
const DEPTH: usize = 8;

const NOT_DRIVES: [&str; 3] = ["loop", "ram", "zram"];

/// Read from the transport and rotation, never entered (MET-027).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DriveKind {
    Nvme,
    Ssd,
    Hdd,
    Disk,
}

impl DriveKind {
    /// The text kept in `metrics.drives.kind` and sent in a batch's `drives`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nvme => "nvme",
            Self::Ssd => "ssd",
            Self::Hdd => "hdd",
            Self::Disk => "disk",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        [Self::Nvme, Self::Ssd, Self::Hdd, Self::Disk]
            .into_iter()
            .find(|kind| kind.as_str() == text)
    }

    fn of(transport: Option<&str>, rotational: Option<bool>) -> Self {
        match (transport, rotational) {
            (Some("nvme"), _) => Self::Nvme,
            (_, Some(false)) => Self::Ssd,
            (_, Some(true)) => Self::Hdd,
            (_, None) => Self::Disk,
        }
    }
}

/// One drive of a device and the operator's mounts on it (MET-027).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drive {
    pub name: String,
    pub kind: DriveKind,
    pub size_bytes: u64,
    /// The mounts on the drive and on what sits on it, sorted bytewise, each once, at most [`MAX_LABELS`].
    pub mounts: Vec<String>,
}

#[derive(Deserialize)]
struct Listing {
    blockdevices: Vec<Node>,
}

/// Every member may be missing or `null`; util-linux before 2.33 printed numbers and flags as strings.
#[derive(Deserialize)]
struct Node {
    name: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    size: Option<Loose>,
    rota: Option<Loose>,
    tran: Option<String>,
    mountpoints: Option<Vec<Option<String>>>,
    mountpoint: Option<String>,
    #[serde(default)]
    children: Vec<Node>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Loose {
    Number(u64),
    Flag(bool),
    Text(String),
}

impl Loose {
    fn number(&self) -> Option<u64> {
        match self {
            Self::Number(number) => Some(*number),
            Self::Text(text) => text.trim().parse().ok(),
            Self::Flag(_) => None,
        }
    }

    fn flag(&self) -> Option<bool> {
        match self {
            Self::Flag(flag) => Some(*flag),
            Self::Number(0) => Some(false),
            Self::Number(1) => Some(true),
            Self::Text(text) => match text.trim() {
                "0" => Some(false),
                "1" => Some(true),
                _ => None,
            },
            Self::Number(_) => None,
        }
    }
}

/// The drives by name (bytewise), each once, at most [`MAX_LABELS`]; `None` when the output does not parse.
#[must_use]
pub fn drives(output: &[u8]) -> Option<Vec<Drive>> {
    let listing: Listing = serde_json::from_slice(output).ok()?;
    let mut drives: Vec<Drive> = listing.blockdevices.iter().filter_map(drive).collect();
    drives.sort_by(|a, b| a.name.cmp(&b.name));
    drives.dedup_by(|later, earlier| later.name == earlier.name);
    drives.truncate(MAX_LABELS);
    Some(drives)
}

fn drive(node: &Node) -> Option<Drive> {
    let name = label(node.name.as_deref()?)?;
    let is_drive = node.kind.as_deref() == Some("disk")
        && !NOT_DRIVES.iter().any(|prefix| name.starts_with(prefix));
    let size_bytes = node.size.as_ref()?.number()?;
    if !is_drive || size_bytes == 0 || i64::try_from(size_bytes).is_err() {
        return None;
    }
    let mut mounts = Vec::new();
    collect_mounts(node, 0, &mut mounts);
    mounts.sort();
    mounts.dedup();
    mounts.truncate(MAX_LABELS);
    Some(Drive {
        name: name.to_owned(),
        kind: DriveKind::of(
            node.tran.as_deref(),
            node.rota.as_ref().and_then(Loose::flag),
        ),
        size_bytes,
        mounts,
    })
}

fn collect_mounts(node: &Node, depth: usize, mounts: &mut Vec<String>) {
    let listed = node.mountpoints.iter().flatten().flatten();
    for mount in listed.chain(&node.mountpoint) {
        if let Some(mount) = label(mount).filter(|mount| wanted(mount)) {
            mounts.push(mount.to_owned());
        }
    }
    if depth < DEPTH {
        for child in &node.children {
            collect_mounts(child, depth + 1, mounts);
        }
    }
}

/// Swap's `[SWAP]` is not a path.
fn wanted(mount: &str) -> bool {
    mount.starts_with('/') && is_operator_mount(mount)
}

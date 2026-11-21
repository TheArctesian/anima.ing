use super::{Error, Result, command};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    os::unix::fs::{FileTypeExt as _, MetadataExt as _},
    path::Path,
};

#[derive(Clone, Deserialize, Serialize)]
pub struct Disk {
    pub path: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub size: u64,
    pub model: Option<String>,
    pub serial: Option<String>,
    #[serde(default)]
    pub ro: bool,
    #[serde(default)]
    pub rm: bool,
    pub tran: Option<String>,
    #[serde(rename = "maj:min")]
    pub number: Option<String>,
    pub label: Option<String>,
    pub partn: Option<u32>,
    #[serde(default)]
    pub mountpoints: Option<Vec<Option<String>>>,
    #[serde(default)]
    pub children: Vec<Self>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eligible: Option<bool>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    path: String,
    #[serde(rename = "maj:min")]
    number: Option<String>,
    size: u64,
    model: Option<String>,
    serial: Option<String>,
}

impl Disk {
    pub fn identity(&self) -> Identity {
        Identity {
            path: self.path.clone(),
            number: self.number.clone(),
            size: self.size,
            model: self.model.clone(),
            serial: self.serial.clone(),
        }
    }
    fn busy(&self) -> bool {
        !matches!(self.kind.as_str(), "disk" | "part")
            || self.mountpoints.as_ref().is_some_and(|mounts| {
                mounts
                    .iter()
                    .any(|m| m.as_ref().is_some_and(|m| !m.is_empty()))
            })
            || self.children.iter().any(Self::busy)
    }
    pub fn allowed(&self) -> bool {
        self.kind == "disk"
            && !self.ro
            && !self.rm
            && self.tran.as_deref() != Some("usb")
            && self.size >= 32 * 1024 * 1024 * 1024
            && !self.busy()
    }
    fn labeled(&self) -> bool {
        matches!(self.label.as_deref(), Some("ANIMA_OS" | "ANIMA_EFI"))
            || self.children.iter().any(Self::labeled)
    }
}

pub fn inventory() -> Result<Vec<Disk>> {
    #[derive(Deserialize)]
    struct Inventory {
        blockdevices: Vec<Disk>,
    }
    let bytes = command::capture(
        "lsblk",
        &[
            "--json",
            "--tree",
            "--bytes",
            "--paths",
            "--output",
            "PATH,TYPE,SIZE,MODEL,SERIAL,RO,MOUNTPOINTS,LABEL,MAJ:MIN,TRAN,RM,PARTN",
        ],
    )?;
    let inventory: Inventory = serde_json::from_slice(&bytes)?;
    if inventory.blockdevices.len() > 128 || inventory.blockdevices.iter().any(|d| d.kind == "part")
    {
        return Err(Error::Refused(
            "Disk inventory is not a bounded dependency tree; refusing installation.",
        ));
    }
    Ok(inventory.blockdevices)
}

pub fn validate(selected: &Disk, devices: &[Disk], labels: bool) -> Result<()> {
    let candidates: Vec<_> = devices
        .iter()
        .filter(|d| d.identity() == selected.identity())
        .collect();
    if candidates.len() != 1 || !candidates[0].allowed() {
        return Err(Error::Refused(
            "Selected disk changed or became busy; review disk selection again.",
        ));
    }
    if labels
        && devices
            .iter()
            .any(|d| d.path != selected.path && d.labeled())
    {
        return Err(Error::Refused(
            "Another disk has Anima labels; disconnect it before installation.",
        ));
    }
    validate_node(selected)
}

fn validate_node(selected: &Disk) -> Result<()> {
    let path = Path::new(&selected.path);
    let metadata = fs::symlink_metadata(path)?;
    if !path.is_absolute() || !metadata.file_type().is_block_device() {
        return Err(Error::Refused(
            "Target must be a real whole block device, not a link or file.",
        ));
    }
    let number = format!(
        "{}:{}",
        rustix::fs::major(metadata.rdev()),
        rustix::fs::minor(metadata.rdev())
    );
    if selected.number.as_deref() != Some(number.as_str()) {
        return Err(Error::Refused(
            "Block device identity changed; no further disk writes are allowed.",
        ));
    }
    Ok(())
}

pub fn validate_all(system: &Disk, data: &[Disk]) -> Result<()> {
    let devices = inventory()?;
    let mut seen = HashSet::new();
    for (index, disk) in std::iter::once(system).chain(data).enumerate() {
        if !seen.insert(&disk.path) {
            return Err(Error::Refused(
                "A disk cannot have multiple installation roles.",
            ));
        }
        validate(disk, &devices, index == 0)?;
    }
    Ok(())
}

pub fn partitions(selected: &Disk, count: usize) -> Result<Vec<String>> {
    for _ in 0..20 {
        let devices = inventory()?;
        validate(selected, &devices, false)?;
        let device = devices
            .iter()
            .find(|disk| disk.identity() == selected.identity())
            .ok_or(Error::Refused(
                "The selected disk disappeared after partitioning.",
            ))?;
        let mut parts: Vec<_> = device
            .children
            .iter()
            .filter(|disk| disk.kind == "part")
            .collect();
        if parts.len() == count {
            parts.sort_by_key(|disk| disk.partn);
            for (index, part) in parts.iter().enumerate() {
                if part.partn.and_then(|number| usize::try_from(number).ok()) != Some(index + 1) {
                    return Err(Error::Refused(
                        "The selected partition numbers are invalid.",
                    ));
                }
                validate_node(part)?;
            }
            return Ok(parts.iter().map(|disk| disk.path.clone()).collect());
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    Err(Error::Refused(
        "The newly selected partitions did not appear; installation stopped.",
    ))
}

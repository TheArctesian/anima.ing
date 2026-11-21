use super::{
    Error, Result, command,
    disks::{self, Disk},
    persist, progress,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Volume {
    pub uuid: Uuid,
    pub model: Option<String>,
    pub serial: Option<String>,
}

pub fn prepare(selected: &[Disk], destination: &Path) -> Result<()> {
    let mut volumes = Vec::new();
    for device in selected {
        progress("Preparing selected data volume", 0.5);
        disks::validate(device, &disks::inventory()?, false)?;
        command::run("wipefs", &["--all", &device.path])?;
        command::run(
            "parted",
            &[
                "--script",
                &device.path,
                "mklabel",
                "gpt",
                "mkpart",
                "AnimaData",
                "btrfs",
                "1MiB",
                "100%",
            ],
        )?;
        command::run("udevadm", &["settle"])?;
        let parts = disks::partitions(device, 1)?;
        command::run("mkfs.btrfs", &["-f", &parts[0]])?;
        let identifier = command::capture("blkid", &["-s", "UUID", "-o", "value", &parts[0]])?;
        let identifier = std::str::from_utf8(&identifier)
            .map_err(|_| Error::Refused("The data filesystem identity is invalid."))?;
        let uuid = Uuid::parse_str(identifier.trim())
            .map_err(|_| Error::Refused("The data filesystem UUID is invalid."))?;
        volumes.push(Volume {
            uuid,
            model: device.model.clone(),
            serial: device.serial.clone(),
        });
    }
    persist::private_write(
        &destination.join("data-volumes.json"),
        &serde_json::to_vec(&volumes)?,
    )
}

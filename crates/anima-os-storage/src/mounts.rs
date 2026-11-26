//! Resolve recorded UUIDs and mount only their verified block devices.
use std::{
    fs::{self, File, OpenOptions, Permissions},
    io,
    os::unix::fs::{FileTypeExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    path::Path,
};

use crate::{Error, PERSIST, RECORD_NAME, Result, VOLUME_MOUNTS, Volume, command, records};

/// Mount the optional data volumes from the persisted installation record.
///
/// # Errors
/// The caller lacks local root authority, the record or mount target is unsafe, or mounting fails.
pub fn mount_recorded() -> Result<()> {
    if !rustix::process::geteuid().is_root() {
        return Err(Error::Refused(
            "Volume recovery requires local root authority.",
        ));
    }
    let persist = Path::new(PERSIST);
    let Some(volumes) = records::read(&persist.join(RECORD_NAME))? else {
        return Ok(());
    };
    let _writer = writer_lock()?;
    let root = Path::new(VOLUME_MOUNTS);
    mount_directory(root)?;
    for volume in volumes {
        mount_one(root, &volume)?;
    }
    Ok(())
}

fn writer_lock() -> Result<File> {
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(
            (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK)
                .bits()
                .cast_signed(),
        )
        .open("/run/anima-data-volumes.lock")?;
    let metadata = lock.metadata()?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err(Error::Refused("The data volume lock is unsafe."));
    }
    lock.try_lock()
        .map_err(|_| Error::Refused("Recorded data volumes are already being mounted."))?;
    Ok(lock)
}

fn mount_one(root: &Path, volume: &Volume) -> Result<()> {
    let identifier = volume.uuid.to_string();
    let source = format!("/dev/disk/by-uuid/{identifier}");
    if !Path::new(&source).exists() {
        println!(
            "{}",
            serde_json::json!({"state":"missing", "uuid":identifier})
        );
        return Ok(());
    }
    if !fs::metadata(&source)?.file_type().is_block_device() {
        return Err(Error::Refused(
            "A recorded volume UUID does not resolve to a block device.",
        ));
    }
    unique_source(&identifier, &source)?;
    let target = root.join(&identifier);
    mount_directory(&target)?;
    let target = target
        .to_str()
        .ok_or(Error::Refused("Invalid volume mount target."))?;
    let mounted = command::optional_capture(
        "findmnt",
        &["--json", "--mountpoint", target, "--output", "UUID"],
    )?;
    if let Some(bytes) = mounted {
        let result: serde_json::Value = serde_json::from_slice(&bytes)?;
        if result["filesystems"][0]["uuid"].as_str() != Some(&identifier) {
            return Err(Error::Refused(
                "A different filesystem occupies a recorded volume mount.",
            ));
        }
        return Ok(());
    }
    command::run(
        "mount",
        &[
            "-t",
            "btrfs",
            "-o",
            "compress=zstd,nosuid,nodev",
            &source,
            target,
        ],
    )
}

fn mount_directory(path: &Path) -> Result<()> {
    match fs::create_dir(path) {
        Ok(()) => fs::set_permissions(path, Permissions::from_mode(0o755))?,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
        || path.canonicalize()? != path
    {
        return Err(Error::Refused("A volume mount directory is unsafe."));
    }
    Ok(())
}

fn unique_source(identifier: &str, source: &str) -> Result<()> {
    let output = command::capture(
        "blkid",
        &[
            "-c",
            "/dev/null",
            "-t",
            &format!("UUID={identifier}"),
            "-o",
            "device",
        ],
        "120s",
    )?;
    let output = std::str::from_utf8(&output)
        .map_err(|_| Error::Refused("Invalid recorded-volume lookup."))?;
    let devices: Vec<_> = output.lines().collect();
    if devices.len() != 1 || fs::metadata(devices[0])?.rdev() != fs::metadata(source)?.rdev() {
        return Err(Error::Refused(
            "A recorded filesystem UUID is ambiguous; disconnect cloned volumes.",
        ));
    }
    Ok(())
}

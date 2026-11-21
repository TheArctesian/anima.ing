use super::{Error, Result, command, persist, volumes::Volume};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Read as _,
    os::unix::fs::{FileTypeExt as _, MetadataExt as _, OpenOptionsExt as _},
    path::Path,
};

pub fn mount() -> Result<()> {
    if !rustix::process::geteuid().is_root() {
        return Err(Error::Refused(
            "Volume recovery requires local root authority.",
        ));
    }
    let records = Path::new("/persist/data-volumes.json");
    if !records.exists() {
        return Ok(());
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(
            (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK)
                .bits()
                .cast_signed(),
        )
        .open(records)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err(Error::Refused(
            "The recorded volume list is not a protected root-owned file.",
        ));
    }
    let mut bytes = Vec::new();
    file.take(16385).read_to_end(&mut bytes)?;
    if bytes.len() > 16384 {
        return Err(Error::Refused("The recorded volume list is too large."));
    }
    let volumes: Vec<Volume> = serde_json::from_slice(&bytes)?;
    let mut seen = HashSet::new();
    if volumes.len() > 16
        || volumes
            .iter()
            .any(|v| v.uuid.is_nil() || !seen.insert(v.uuid))
    {
        return Err(Error::Refused(
            "The recorded volume identities are invalid or repeated.",
        ));
    }
    let root = Path::new("/persist/volumes");
    if !root.exists() {
        persist::directory(root, 0o755)?;
    }
    if root.canonicalize()? != root || !protected_directory(root)? {
        return Err(Error::Refused(
            "The volume directory must not follow a symbolic link.",
        ));
    }
    for volume in volumes {
        mount_one(root, &volume)?;
    }
    Ok(())
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
    if !target.exists() {
        persist::directory(&target, 0o755)?;
    }
    if target.canonicalize()? != target || !protected_directory(&target)? {
        return Err(Error::Refused(
            "A volume mount target follows a symbolic link.",
        ));
    }
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

fn protected_directory(path: &Path) -> Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    Ok(metadata.is_dir() && metadata.uid() == 0 && metadata.mode() & 0o022 == 0)
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

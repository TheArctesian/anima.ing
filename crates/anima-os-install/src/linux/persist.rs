use super::{Error, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    path::Path,
};

pub fn directory(path: &Path, mode: u32) -> Result<()> {
    fs::create_dir(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    if path.canonicalize()? != path {
        return Err(Error::Refused(
            "Installation directories must not follow symbolic links.",
        ));
    }
    Ok(())
}

pub fn private_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn read_owned(path: &Path, limit: u64) -> Result<Option<Vec<u8>>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.uid() != 0 {
        return Ok(None);
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(
            (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK)
                .bits()
                .cast_signed(),
        )
        .open(path)?;
    let current = file.metadata()?;
    if !current.is_file()
        || current.uid() != 0
        || current.ino() != metadata.ino()
        || current.dev() != metadata.dev()
    {
        return Err(Error::Refused(
            "A retained settings file changed during installation.",
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(Error::Refused(
            "A retained settings file exceeds its size limit.",
        ));
    }
    Ok(Some(bytes))
}

pub fn network(destination: &Path) -> Result<()> {
    let source = Path::new("/etc/NetworkManager/system-connections");
    if !source.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if entry
            .path()
            .extension()
            .is_none_or(|extension| extension != "nmconnection")
        {
            continue;
        }
        if let Some(bytes) = read_owned(&entry.path(), 1024 * 1024)? {
            private_write(&destination.join(entry.file_name()), &bytes)?;
        }
    }
    Ok(())
}

pub fn display(destination: &Path) -> Result<()> {
    let source = Path::new("/run/anima-display.json");
    if !source.exists() {
        return Ok(());
    }
    if let Some(bytes) = read_owned(source, 16384)? {
        let _: serde_json::Value = serde_json::from_slice(&bytes)?;
        private_write(destination, &bytes)?;
        fs::set_permissions(destination, fs::Permissions::from_mode(0o644))?;
    }
    Ok(())
}

pub fn reduced_motion(destination: &Path) -> Result<()> {
    match fs::symlink_metadata("/run/anima-reduced-motion") {
        Ok(metadata) if metadata.is_file() && metadata.uid() == 0 => {
            private_write(destination, b"")
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub fn writer_lock() -> Result<File> {
    let file = OpenOptions::new()
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
        .open("/run/anima-install-operation.lock")?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.uid() != 0 {
        return Err(Error::Refused(
            "The installation lock is not a root-owned regular file.",
        ));
    }
    file.try_lock()
        .map_err(|_| Error::Refused("Another installation is already running."))?;
    Ok(file)
}

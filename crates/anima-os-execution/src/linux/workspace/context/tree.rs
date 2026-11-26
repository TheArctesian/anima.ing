use super::{Error, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 50_000;
#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;

pub struct Tree {
    digest: Sha256,
    pub bytes: u64,
    pub entries: usize,
    scanned: usize,
    source: PathBuf,
    destination: PathBuf,
}

impl Tree {
    pub fn capture(source: &Path, destination: &Path) -> Result<Self> {
        if !fs::symlink_metadata(source)?.is_dir() {
            return Err(Error::Refused("Workspace source must be a real directory."));
        }
        let mut tree = Self {
            digest: Sha256::new(),
            bytes: 0,
            entries: 0,
            scanned: 0,
            source: source.to_owned(),
            destination: destination.to_owned(),
        };
        tree.digest.update(b"anima-build-context-v1\0");
        tree.walk(Path::new(""))?;
        Ok(tree)
    }

    pub fn digest(self) -> String {
        format!(
            "sha256:{}",
            crate::linux::digests::hex(&self.digest.finalize())
        )
    }

    fn field(&mut self, value: &[u8]) {
        self.digest.update((value.len() as u64).to_be_bytes());
        self.digest.update(value);
    }

    fn walk(&mut self, relative: &Path) -> Result<()> {
        let source = self.source.join(relative);
        let destination = self.destination.join(relative);
        if relative.components().count() > 64 {
            return Err(Error::Refused(
                "Source directory nesting exceeded its limit.",
            ));
        }
        let mut entries = fs::read_dir(source)?
            .take(MAX_ENTRIES.saturating_sub(self.scanned) + 1)
            .collect::<std::io::Result<Vec<_>>>()?;
        self.scanned += entries.len();
        if self.scanned > MAX_ENTRIES {
            return Err(Error::Refused("Source directory has too many entries."));
        }
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let name = entry.file_name();
            let raw = name.as_bytes();
            if raw.starts_with(b".env") || matches!(raw, b".git" | b".ssh" | b".aws" | b".config") {
                continue;
            }
            self.entries += 1;
            if self.entries > MAX_ENTRIES {
                return Err(Error::Refused("Source context has too many entries."));
            }
            let path = entry.path();
            let relative = relative.join(&name);
            let target = destination.join(&name);
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.is_dir() {
                self.field(relative.as_os_str().as_bytes());
                self.field(b"directory");
                fs::create_dir(&target)?;
                fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
                self.walk(&relative)?;
            } else if metadata.is_file() {
                self.copy_file(&relative, &metadata)?;
            } else if metadata.file_type().is_symlink() {
                self.copy_link(&relative)?;
            } else {
                return Err(Error::Refused(
                    "Source context contains a device, socket or other unsupported entry.",
                ));
            }
        }
        fs::File::open(destination)?.sync_all()?;
        Ok(())
    }
    fn copy_file(&mut self, relative: &Path, metadata: &fs::Metadata) -> Result<()> {
        let path = self.source.join(relative);
        let target = self.destination.join(relative);
        self.bytes = self
            .bytes
            .checked_add(metadata.len())
            .filter(|size| *size <= MAX_BYTES)
            .ok_or(Error::Refused(
                "Source context exceeds the 2 GiB capture limit.",
            ))?;
        let mode: u32 = if metadata.permissions().mode() & 0o111 != 0 {
            0o755
        } else {
            0o644
        };
        self.field(relative.as_os_str().as_bytes());
        self.field(b"file");
        self.field(&mode.to_be_bytes());
        self.field(&metadata.len().to_be_bytes());
        let mut input = fs::OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(path)?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&target)?;
        let mut content = Sha256::new();
        let mut copied = 0_u64;
        let mut buffer = vec![0_u8; 65536];
        loop {
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            copied += count as u64;
            if copied > metadata.len() {
                return Err(Error::Refused("Source changed during immutable capture."));
            }
            content.update(&buffer[..count]);
            output.write_all(&buffer[..count])?;
        }
        if copied != metadata.len() {
            return Err(Error::Refused("Source changed during immutable capture."));
        }
        fs::set_permissions(&target, fs::Permissions::from_mode(mode))?;
        output.sync_all()?;
        self.field(&content.finalize());
        Ok(())
    }

    fn copy_link(&mut self, relative: &Path) -> Result<()> {
        let path = self.source.join(relative);
        let target = self.destination.join(relative);
        let link = fs::read_link(path)?;
        // Keep links inside the exported tree. Absolute/parent links can
        // otherwise make a later tool inspect outside its source context.
        if link.is_absolute()
            || link
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(Error::Refused(
                "Source context contains a link outside its directory.",
            ));
        }
        if link
            .components()
            .any(|part| part.as_os_str().as_bytes().starts_with(b".env"))
        {
            return Ok(());
        }
        self.field(relative.as_os_str().as_bytes());
        self.field(b"symlink");
        self.field(link.as_os_str().as_bytes());
        std::os::unix::fs::symlink(link, target)?;
        Ok(())
    }
}

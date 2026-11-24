use std::fs::File;
use std::io::Read as _;
use std::path::{Component, Path};

use daemon_core::inventory::mobile::{self, Entries, Probe, SdkPackage, State};
use rustix::fd::OwnedFd;
use rustix::fs::{Dir, FileType, Mode, OFlags};

const DIRECTORY: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
const ROOTS: &[&[u8]] = &[
    b"platforms",
    b"system-images",
    b"build-tools",
    b"cmdline-tools",
    b"platform-tools",
    b"emulator",
];

struct Scan {
    entries: usize,
    bytes: usize,
    manifests: Vec<Vec<u8>>,
}

pub(super) fn read(root: &Path) -> Probe<Entries<SdkPackage>> {
    match scan(root) {
        Ok(manifests) => mobile::sdk_packages(&manifests),
        Err(state) => Probe::Unavailable(state),
    }
}

fn scan(root: &Path) -> Result<Vec<Vec<u8>>, State> {
    let root = open_root(root)?;
    let mut scan = Scan {
        entries: 0,
        bytes: 0,
        manifests: vec![],
    };
    scan.walk(&root, 0)?;
    Ok(scan.manifests)
}

fn open_root(root: &Path) -> Result<OwnedFd, State> {
    if !root.is_absolute() {
        return Err(State::Exited);
    }
    let mut fd = rustix::fs::open("/", DIRECTORY, Mode::empty()).map_err(|_| State::Exited)?;
    for part in root.components() {
        match part {
            Component::RootDir => {}
            Component::Normal(name) => {
                fd = rustix::fs::openat(&fd, name, DIRECTORY, Mode::empty()).map_err(|error| {
                    if error == rustix::io::Errno::NOENT {
                        State::Missing
                    } else {
                        State::Exited
                    }
                })?;
            }
            _ => return Err(State::Exited),
        }
    }
    Ok(fd)
}

impl Scan {
    fn walk(&mut self, parent: &OwnedFd, depth: usize) -> Result<(), State> {
        let mut children = Vec::new();
        let entries = Dir::read_from(parent).map_err(|_| State::Exited)?;
        for entry in entries {
            let entry = entry.map_err(|_| State::Exited)?;
            let name = entry.file_name();
            if matches!(name.to_bytes(), b"." | b"..") {
                continue;
            }
            self.entries += 1;
            if self.entries > 4096 {
                return Err(State::TooLarge);
            }
            if depth == 0 && !ROOTS.contains(&name.to_bytes()) {
                continue;
            }
            if name.to_bytes() == b"package.xml" {
                // A package's contents are not inventory. Do not traverse its images or tools.
                return self.manifest(parent, name);
            }
            children.push(name.to_owned());
        }
        for name in children {
            let name = name.as_c_str();
            let stat = rustix::fs::statat(parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|_| State::Exited)?;
            let kind = FileType::from_raw_mode(stat.st_mode);
            if kind == FileType::Symlink {
                return Err(State::Exited);
            }
            if kind == FileType::Directory {
                if depth >= 8 {
                    return Err(State::TooLarge);
                }
                let child = rustix::fs::openat(parent, name, DIRECTORY, Mode::empty())
                    .map_err(|_| State::Exited)?;
                self.walk(&child, depth + 1)?;
            }
        }
        Ok(())
    }

    fn manifest(&mut self, parent: &OwnedFd, name: &std::ffi::CStr) -> Result<(), State> {
        if self.manifests.len() >= 512 {
            return Err(State::TooLarge);
        }
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        let fd =
            rustix::fs::openat(parent, name, flags, Mode::empty()).map_err(|_| State::Exited)?;
        let stat = rustix::fs::fstat(&fd).map_err(|_| State::Exited)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile || stat.st_nlink != 1 {
            return Err(State::Exited);
        }
        let mut bytes = Vec::new();
        File::from(fd)
            .take((mobile::INPUT_LIMIT - self.bytes + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| State::Exited)?;
        self.bytes += bytes.len();
        if self.bytes > mobile::INPUT_LIMIT {
            return Err(State::TooLarge);
        }
        self.manifests.push(bytes);
        Ok(())
    }
}

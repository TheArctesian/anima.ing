//! Bounded, non-link directory discovery for the two session layouts (THR-007, THR-058).
use nix::{
    fcntl::{OFlag, openat},
    sys::stat::Mode,
};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io,
    os::unix::ffi::OsStrExt as _,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    path::{Path, PathBuf},
};

struct Budget {
    home: PathBuf,
    left: usize,
    #[cfg(target_os = "macos")]
    prefix: &'static str,
}

const MOST_FOLDERS: usize = 1024;
const MOST_ENTRIES: usize = 16_384;

pub(super) fn discover(home: &Path) -> io::Result<BTreeMap<PathBuf, (u64, u64)>> {
    let mut folders = BTreeMap::new();
    insert(home, &mut folders)?;
    let mut budget = Budget {
        home: home.to_path_buf(),
        left: MOST_ENTRIES,
        #[cfg(target_os = "macos")]
        prefix: "",
    };
    for (program, sessions, depth) in [(".claude", "projects", 1), (".codex", "sessions", 3)] {
        #[cfg(target_os = "macos")]
        {
            budget.prefix = if program == ".codex" { "rollout-" } else { "" };
        }
        let parent = home.join(program);
        if !insert(&parent, &mut folders)? {
            continue;
        }
        let root = parent.join(sessions);
        if insert(&root, &mut folders)? {
            descend(&root, depth, &mut folders, &mut budget)?;
        }
    }
    Ok(folders)
}

fn insert(path: &Path, folders: &mut BTreeMap<PathBuf, (u64, u64)>) -> io::Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if !metadata.is_dir() {
        return Ok(false);
    }
    if folders.len() == MOST_FOLDERS {
        return Err(io::Error::other("session watch folder limit"));
    }
    folders.insert(path.to_path_buf(), (metadata.dev(), metadata.ino()));
    Ok(true)
}

fn descend(
    path: &Path,
    depth: usize,
    folders: &mut BTreeMap<PathBuf, (u64, u64)>,
    budget: &mut Budget,
) -> io::Result<()> {
    let identity = *folders
        .get(path)
        .ok_or_else(|| io::Error::other("untracked watch folder"))?;
    let directory = open(&budget.home, path, identity)?;
    let entries = rustix::fs::Dir::read_from(&directory)?;
    for entry in entries {
        let entry = entry?;
        let name = std::ffi::OsStr::from_bytes(entry.file_name().to_bytes());
        if name == "." || name == ".." {
            continue;
        }
        if budget.left == 0 {
            return Err(io::Error::other("session watch entry limit"));
        }
        budget.left -= 1;
        let metadata =
            nix::sys::stat::fstatat(&directory, name, nix::fcntl::AtFlags::AT_SYMLINK_NOFOLLOW)?;
        let kind = nix::sys::stat::SFlag::from_bits_truncate(metadata.st_mode);
        let logical = path.join(name);
        if (kind & nix::sys::stat::SFlag::S_IFMT) == nix::sys::stat::SFlag::S_IFDIR && depth > 0 {
            record(
                &logical,
                (device(metadata.st_dev), metadata.st_ino),
                folders,
            )?;
            descend(&logical, depth - 1, folders, budget)?;
        }
        #[cfg(target_os = "macos")]
        if (kind & nix::sys::stat::SFlag::S_IFMT) == nix::sys::stat::SFlag::S_IFREG
            && (depth == 0 || !budget.prefix.is_empty())
            && name.to_string_lossy().starts_with(budget.prefix)
            && name.to_string_lossy().ends_with(".jsonl")
            && metadata.st_nlink == 1
            && metadata.st_uid == nix::unistd::getuid().as_raw()
        {
            record(
                &logical,
                (device(metadata.st_dev), metadata.st_ino),
                folders,
            )?;
        }
    }
    Ok(())
}

/// No path component can redirect the watch outside the supplied home. No FIFO can block it.
pub(super) fn open(home: &Path, path: &Path, identity: (u64, u64)) -> io::Result<File> {
    let flags = OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags((flags | OFlag::O_DIRECTORY).bits())
        .open(home)?;
    for component in path
        .strip_prefix(home)
        .map_err(io::Error::other)?
        .components()
    {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err(io::Error::other("invalid watch path"));
        }
        file = File::from(openat(&file, component.as_os_str(), flags, Mode::empty())?);
    }
    let metadata = file.metadata()?;
    if (metadata.dev(), metadata.ino()) != identity
        || !(metadata.is_dir()
            || (metadata.is_file()
                && metadata.nlink() == 1
                && metadata.uid() == nix::unistd::getuid().as_raw()))
    {
        return Err(io::Error::other("session watch inode changed"));
    }
    Ok(file)
}

fn record(
    path: &Path,
    identity: (u64, u64),
    folders: &mut BTreeMap<PathBuf, (u64, u64)>,
) -> io::Result<()> {
    if folders.len() == MOST_FOLDERS {
        return Err(io::Error::other("session watch path limit"));
    }
    folders.insert(path.to_path_buf(), identity);
    Ok(())
}

#[cfg(target_os = "macos")]
fn device(value: nix::libc::dev_t) -> u64 {
    u64::from(value.cast_unsigned())
}
#[cfg(not(target_os = "macos"))]
const fn device(value: nix::libc::dev_t) -> u64 {
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn discovery_rejects_replacement_before_enumeration() {
        let home = tempfile::tempdir().expect("synthetic home");
        let outside = tempfile::tempdir().expect("synthetic outside");
        let root = home.path().join("sessions");
        fs::create_dir(&root).expect("session root");
        fs::create_dir(outside.path().join("outside-only")).expect("outside marker");
        let mut folders = BTreeMap::new();
        assert!(insert(&root, &mut folders).expect("validated root"));
        fs::rename(&root, home.path().join("retired")).expect("replace root");
        symlink(outside.path(), &root).expect("redirect root");
        let mut budget = Budget {
            home: home.path().to_path_buf(),
            left: MOST_ENTRIES,
            #[cfg(target_os = "macos")]
            prefix: "",
        };
        assert!(descend(&root, 1, &mut folders, &mut budget).is_err());
        assert_eq!(budget.left, MOST_ENTRIES, "no outside entry enumerated");
        assert_eq!(folders.len(), 1, "no outside directory discovered");
    }
}

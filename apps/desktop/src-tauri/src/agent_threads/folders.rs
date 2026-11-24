//! Bounded, non-link directory discovery for the two session layouts (THR-007, THR-058).
use std::{
    collections::BTreeMap,
    fs, io,
    os::unix::fs::MetadataExt as _,
    path::{Path, PathBuf},
};

struct Budget {
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
    for entry in fs::read_dir(path)? {
        if budget.left == 0 {
            return Err(io::Error::other("session watch entry limit"));
        }
        budget.left -= 1;
        let entry = entry?;
        if entry.file_type()?.is_dir() && depth > 0 && insert(&entry.path(), folders)? {
            descend(&entry.path(), depth - 1, folders, budget)?;
        }
        #[cfg(target_os = "macos")]
        if entry.file_type()?.is_file()
            && (depth == 0 || !budget.prefix.is_empty())
            && entry
                .file_name()
                .to_string_lossy()
                .starts_with(budget.prefix)
            && entry.file_name().to_string_lossy().ends_with(".jsonl")
        {
            let metadata = entry.metadata()?;
            if metadata.nlink() == 1 && metadata.uid() == nix::unistd::getuid().as_raw() {
                if folders.len() == MOST_FOLDERS {
                    return Err(io::Error::other("session watch path limit"));
                }
                folders.insert(entry.path(), (metadata.dev(), metadata.ino()));
            }
        }
    }
    Ok(())
}

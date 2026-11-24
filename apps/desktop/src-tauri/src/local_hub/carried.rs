use std::path::{Path, PathBuf};

use super::LocalHubError;
use crate::carried::{DevFolders, programs_folder};

const PROGRAM: &str = "anima-local-hub";
/// The tree below `usr/` of the Linux bundle: Tauri puts resources in `lib/<productName>/`.
pub(super) const LINUX_TREE: &str = "lib/Anima.ing/database";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carried {
    pub program: PathBuf,
    pub tree: PathBuf,
}

/// Only a release build uses the programs beside `executable`: a debug target folder may hold programs of another commit.
pub fn beside(
    executable: &Path,
    macos: bool,
    release: bool,
    dev: &DevFolders,
) -> Result<Carried, LocalHubError> {
    let folder = programs_folder(executable, release, dev).ok_or(LocalHubError::NotCarried)?;
    let tree = if release {
        let above = folder.parent().ok_or(LocalHubError::NotCarried)?;
        if cfg!(windows) {
            folder.join("database")
        } else if macos {
            above.join("Resources/database")
        } else {
            above.join(LINUX_TREE)
        }
    } else {
        dev.tree
            .clone()
            .filter(|tree| tree.is_absolute())
            .ok_or(LocalHubError::NotCarried)?
    };
    let carried = Carried {
        program: folder.join(if cfg!(windows) {
            "anima-local-hub.exe"
        } else {
            PROGRAM
        }),
        tree,
    };
    let is_there = carried.program.is_file()
        && carried
            .tree
            .join(if cfg!(windows) {
                "bin/postgres.exe"
            } else {
                "bin/postgres"
            })
            .is_file();
    if !is_there {
        return Err(LocalHubError::NotCarried);
    }
    Ok(carried)
}

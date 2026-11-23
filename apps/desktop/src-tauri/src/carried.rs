//! A release build finds the carried programs beside its executable only and never reads
//! `ANIMA_DEV_PROGRAMS` or `ANIMA_DEV_TREE`.

pub mod run;

#[cfg(all(test, unix))]
mod tests;

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevFolders {
    pub programs: Option<PathBuf>,
    pub tree: Option<PathBuf>,
}

impl DevFolders {
    #[cfg(debug_assertions)]
    pub fn of_this_run() -> Self {
        let read = |name| std::env::var_os(name).map(PathBuf::from);
        Self {
            programs: read("ANIMA_DEV_PROGRAMS"),
            tree: read("ANIMA_DEV_TREE"),
        }
    }

    #[cfg(not(debug_assertions))]
    pub fn of_this_run() -> Self {
        Self::default()
    }
}

pub fn programs_folder(executable: &Path, release: bool, dev: &DevFolders) -> Option<PathBuf> {
    if release {
        return executable.parent().map(Path::to_path_buf);
    }
    dev.programs
        .as_deref()
        .filter(|folder| folder.is_absolute())
        .map(Path::to_path_buf)
}

pub fn program(name: &str) -> Option<PathBuf> {
    if !cfg!(any(
        target_os = "macos",
        target_os = "linux",
        target_os = "windows"
    )) {
        return None;
    }
    let executable = std::env::current_exe().ok()?;
    let folder = programs_folder(
        &executable,
        !cfg!(debug_assertions),
        &DevFolders::of_this_run(),
    )?;
    let name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    Some(folder.join(name)).filter(|program| program.is_file())
}

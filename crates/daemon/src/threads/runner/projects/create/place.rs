use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use agent_threads::control::Refusal;
use rustix::fs::{Mode, OFlags, RenameFlags};

use super::super::refused;

const FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

pub(super) struct Destination {
    folder: PathBuf,
    parent: OwnedFd,
}

pub(in super::super) struct Project {
    folder: PathBuf,
    opened: OwnedFd,
}

impl Destination {
    pub(super) fn open(folder: PathBuf) -> Result<Self, Refusal> {
        let parent = rustix::fs::open(
            folder.parent().ok_or(Refusal::FolderNotPermitted)?,
            FLAGS,
            Mode::empty(),
        )
        .map_err(|_| Refusal::FolderNotPermitted)?;
        let destination = Self { folder, parent };
        destination.check()?;
        match rustix::fs::statat(
            &destination.parent,
            destination
                .folder
                .file_name()
                .ok_or(Refusal::FolderNotPermitted)?,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(_) => return Err(path_exists()),
            Err(rustix::io::Errno::NOENT) => {}
            Err(_) => return Err(Refusal::FolderNotPermitted),
        }
        Ok(destination)
    }

    fn check(&self) -> Result<(), Refusal> {
        if opened_path(&self.parent).as_deref() != self.folder.parent() {
            return Err(Refusal::FolderNotPermitted);
        }
        Ok(())
    }

    pub(super) fn publish(self, staged: &Path) -> Result<Project, Refusal> {
        self.check()?;
        let opened = rustix::fs::open(staged, FLAGS, Mode::empty())
            .map_err(|_| refused("The prepared project could not be opened."))?;
        rustix::fs::renameat_with(
            rustix::fs::CWD, staged, &self.parent,
            self.folder.file_name().ok_or(Refusal::FolderNotPermitted)?, RenameFlags::NOREPLACE,
        ).map_err(|error| match error {
            rustix::io::Errno::EXIST => path_exists(),
            rustix::io::Errno::XDEV => refused("The new project is on a different filesystem from Anima. Prepare it with Git, then add it as a local project."),
            _ => refused("The new project could not be placed safely. Check the parent folder and its permissions."),
        })?;
        let project = Project {
            folder: self.folder,
            opened,
        };
        project.folder()?;
        Ok(project)
    }
}

fn path_exists() -> Refusal {
    refused(
        "That path already exists. Choose a new folder, or add the existing folder as a local project.",
    )
}

impl Project {
    pub(in super::super) fn folder(&self) -> Result<PathBuf, Refusal> {
        let expected = rustix::fs::fstat(&self.opened).map_err(|_| Refusal::FolderNotPermitted)?;
        let actual = rustix::fs::statat(
            rustix::fs::CWD,
            &self.folder,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| Refusal::FolderNotPermitted)?;
        if expected.st_dev != actual.st_dev
            || expected.st_ino != actual.st_ino
            || opened_path(&self.opened).as_deref() != Some(self.folder.as_path())
        {
            return Err(Refusal::FolderNotPermitted);
        }
        Ok(self.folder.clone())
    }
}

fn opened_path(open: &OwnedFd) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStringExt as _;
        Some(PathBuf::from(std::ffi::OsString::from_vec(
            rustix::fs::getpath(open).ok()?.into_bytes(),
        )))
    }
    #[cfg(not(target_os = "macos"))]
    {
        use std::os::fd::AsRawFd as _;
        std::fs::read_link(format!("/proc/self/fd/{}", open.as_raw_fd())).ok()
    }
}

//! F-2, F-7.

use std::{
    fs::{self, DirBuilder, OpenOptions},
    io::{self, ErrorKind, Read, Write},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

const PORT_FILE: &str = "anima-port";
/// Its presence says that a unit of the user must keep the link of this folder (HMD-058).
const UNIT_FILE: &str = "anima-unit";
/// More than a port number and a line end.
const MOST_PORT_FILE: u64 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkFolders {
    pub device: PathBuf,
    pub next: PathBuf,
    old: PathBuf,
}

impl LinkFolders {
    pub fn below(application: &Path) -> Self {
        let link = application.join("link");
        Self {
            device: link.join("device"),
            next: link.join("device.next"),
            old: link.join("device.old"),
        }
    }

    pub fn prepare_next(&self) -> io::Result<()> {
        let link = self.device.parent().ok_or(ErrorKind::InvalidInput)?;
        let mut builder = DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        builder.mode(0o700);
        builder.create(link)?;
        if !fs::symlink_metadata(link)?.is_dir() {
            return Err(ErrorKind::InvalidInput.into());
        }
        #[cfg(unix)]
        fs::set_permissions(link, fs::Permissions::from_mode(0o700))?;
        remove_folder(&self.next)
    }

    /// When the swap fails, the folder before is back at `device`.
    pub fn take_next(&self) -> io::Result<()> {
        self.take_next_with(|from, to| fs::rename(from, to))
    }

    pub fn take_next_with(
        &self,
        rename: impl Fn(&Path, &Path) -> io::Result<()>,
    ) -> io::Result<()> {
        self.recover()?;
        remove_folder(&self.old)?;
        let moved_aside = match rename(&self.device, &self.old) {
            Ok(()) => true,
            Err(error) if error.kind() == ErrorKind::NotFound => false,
            Err(error) => return Err(error),
        };
        if let Err(error) = rename(&self.next, &self.device) {
            if moved_aside {
                rename(&self.old, &self.device)?;
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn remove_old(&self) -> io::Result<()> {
        remove_folder(&self.old)
    }

    /// A swap that stopped half way leaves `device` missing and `device.old` there.
    pub fn recover(&self) -> io::Result<()> {
        let missing = matches!(
            fs::symlink_metadata(&self.device),
            Err(error) if error.kind() == ErrorKind::NotFound
        );
        if missing && fs::symlink_metadata(&self.old).is_ok_and(|found| found.is_dir()) {
            fs::rename(&self.old, &self.device)?;
        }
        Ok(())
    }

    pub fn remove(&self) -> io::Result<()> {
        remove_folder(&self.next)?;
        remove_folder(&self.old)?;
        remove_folder(&self.device)
    }
}

/// A symbolic link is removed, not followed.
fn remove_folder(folder: &Path) -> io::Result<()> {
    match fs::symlink_metadata(folder) {
        Ok(found) if found.is_dir() => fs::remove_dir_all(folder),
        Ok(_) => fs::remove_file(folder),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn parse_port(text: &[u8]) -> Option<u16> {
    let digits = text.strip_suffix(b"\n").unwrap_or(text);
    let fits = !digits.is_empty() && digits.len() <= 5 && digits.iter().all(u8::is_ascii_digit);
    if !fits {
        return None;
    }
    std::str::from_utf8(digits)
        .ok()?
        .parse()
        .ok()
        .filter(|port| *port != 0)
}

/// A symbolic link is not followed.
pub fn read_port(folder: &Path) -> Option<u16> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(nix::libc::O_NOFOLLOW);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options.open(folder.join(PORT_FILE)).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut text = Vec::new();
    file.take(MOST_PORT_FILE).read_to_end(&mut text).ok()?;
    parse_port(&text)
}

pub fn write_port(folder: &Path, port: u16) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(folder.join(PORT_FILE))?;
    writeln!(file, "{port}")?;
    file.sync_all()
}

pub fn want_unit(folder: &Path) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let made = options.open(folder.join(UNIT_FILE));
    match made {
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => Err(error),
        _ => Ok(()),
    }
}

/// A link at the place of the mark is no mark.
pub fn wants_unit(folder: &Path) -> bool {
    fs::symlink_metadata(folder.join(UNIT_FILE)).is_ok_and(|mark| mark.is_file())
}

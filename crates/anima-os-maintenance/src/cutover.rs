//! Coordinate persisted Hosting authority with Anima OS generation transitions.
mod boot_menu;
pub use boot_menu::{require_entry, require_menu, valid_system};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read as _},
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    path::Path,
};

use nix::unistd::Uid;
use serde::Deserialize;
use uuid::Uuid;

use super::{Admission, Error};

pub const APPLICATION_ROOT: &str = "/persist/anima-applications";
pub const BOOT_ENTRIES: &str = "/boot/loader/entries";
pub const AUTHORIZATION_BYTES: u64 = 16 * 1024;
const GENERATION_MARKER: &str = "anima-hosting-cutover";
const CAPTURED_MARKER: &str = "anima-hosting-cutover-captured";
const UPDATE_STATE: &str = "/persist/anima-updates/state.json";
const CURRENT_SYSTEM: &str = "/run/current-system";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Format {
    Retained = 1,
    Captured = 2,
}

impl Format {
    #[must_use]
    pub const fn from_schema(schema: u32) -> Option<Self> {
        match schema {
            1 => Some(Self::Retained),
            2 => Some(Self::Captured),
            _ => None,
        }
    }
}

/// Exclude a managed boot transition while installing new Hosting authority.
///
/// # Errors
/// Maintenance is active, the current generation is unsupported, or a boot trial is unresolved.
pub fn authorize(application_root: &Path, format: Format) -> Result<Option<Admission>, Error> {
    if application_root != Path::new(APPLICATION_ROOT) {
        return Ok(None);
    }
    let guard = Admission::acquire()?;
    let format = required_format(application_root)?.map_or(format, |present| present.max(format));
    require_generation(Path::new(&current_generation()?), format)?;
    if let Some(state) = private_json::<Update>(Path::new(UPDATE_STATE), 65_536)?
        && !matches!(
            state.phase.as_str(),
            "idle" | "current" | "staged" | "rolled-back"
        )
    {
        return Err(Error::Refused(
            "Finish the current boot trial before authorizing a Hosting cutover.",
        ));
    }
    require_menu(Path::new(BOOT_ENTRIES), format)?;
    Ok(Some(guard))
}

/// # Errors
/// The running generation could not be resolved to an `AnimaOS` system closure.
pub fn current_generation() -> Result<String, Error> {
    let current = fs::canonicalize(CURRENT_SYSTEM)?;
    current
        .to_str()
        .filter(|system| valid_system(system))
        .map(str::to_owned)
        .ok_or(Error::Refused("The running AnimaOS generation is invalid."))
}

#[derive(Deserialize)]
struct Update {
    phase: String,
}

#[derive(Deserialize)]
struct Authorization {
    schema: u32,
    application: Uuid,
}

/// Read the highest format of the root-owned authorization ledger used by Hosting.
///
/// # Errors
/// A corrupt, untrusted or unsupported record requires local recovery.
pub fn required_format(application_root: &Path) -> Result<Option<Format>, Error> {
    if !private_directory(application_root)? {
        return Ok(None);
    }
    let ledger = application_root.join("cutovers");
    if !private_directory(&ledger)? {
        return Ok(None);
    }
    let mut required = None::<Format>;
    for entry in fs::read_dir(&ledger)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(invalid)?;
        if temporary(name) {
            continue;
        }
        let id = name
            .strip_suffix(".json")
            .and_then(|id| Uuid::parse_str(id).ok())
            .filter(|id| !id.is_nil())
            .ok_or_else(invalid)?;
        let authorization: Authorization =
            private_json(&entry.path(), AUTHORIZATION_BYTES)?.ok_or_else(invalid)?;
        let format = Format::from_schema(authorization.schema).ok_or_else(invalid)?;
        if authorization.application != id {
            return Err(invalid());
        }
        required = Some(required.map_or(format, |current| current.max(format)));
    }
    Ok(required)
}

/// Require a system closure that can honor the selected Hosting cutover record.
///
/// # Errors
/// The generation lacks the declared compatibility.
pub fn require_generation(system: &Path, format: Format) -> Result<(), Error> {
    if !marker(&system.join(GENERATION_MARKER), b"1\n")
        || (format == Format::Captured && !marker(&system.join(CAPTURED_MARKER), b"2\n"))
    {
        return Err(Error::Refused(
            "This OS generation cannot honor retained Hosting cutovers. Choose a compatible generation.",
        ));
    }
    Ok(())
}

fn marker(path: &Path, expected: &[u8]) -> bool {
    let mut value = Vec::new();
    open(path)
        .and_then(|file| {
            if !file.metadata()?.is_file() {
                return Err(io::Error::from(io::ErrorKind::InvalidData));
            }
            file.take(3).read_to_end(&mut value)
        })
        .is_ok()
        && value == expected
}

fn temporary(name: &str) -> bool {
    name.strip_prefix(".write-")
        .is_some_and(|hash| hash.len() == 32 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn private_directory(path: &Path) -> Result<bool, Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.is_dir()
                && metadata.uid() == Uid::effective().as_raw()
                && metadata.mode().trailing_zeros() >= 6 =>
        {
            Ok(true)
        }
        Ok(_) => Err(invalid()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn private_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    limit: u64,
) -> Result<Option<T>, Error> {
    let file = match open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != Uid::effective().as_raw()
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1
    {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid());
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| invalid())
}

fn open(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
}

fn invalid() -> Error {
    Error::Refused(
        "Hosting cutover state is untrusted or unsupported. Inspect the retained state before retrying.",
    )
}

//! SFTP on the device (K4 Section 9). Nothing here logs; the device's own words are dropped,
//! so an answer becomes a K4 refusal or [`Broken`], never text (LOG-007).

use std::time::Duration;

use chrono::DateTime;
use russh::{ChannelStream, client::Msg};
use russh_sftp::{
    client::{Config, RawSftpSession, error::Error as SftpError},
    protocol::{File, FileAttributes, OpenFlags, StatusCode},
};

use super::protocol::{DevicePath, Entry, Kind, Listing, MAX_PATH_BYTES, Refusal};

const MAX_PACKET: u32 = 256 * 1024;
/// Longer names are left out, which bounds a listing's size.
const MAX_NAME_BYTES: usize = 1024;
const TYPE_BITS: u32 = 0o170_000;
const REGULAR: u32 = 0o100_000;
const DIRECTORY: u32 = 0o040_000;
const SYMLINK: u32 = 0o120_000;

/// Holds nothing from the device (LOG-007).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum Broken {
    #[error("the SFTP session with the device failed")]
    Sftp,
    #[error("the device's settings could not be read")]
    Devices,
}

pub(super) type Asked<T> = Result<Result<T, Refusal>, Broken>;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Handle(String);

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Handle")
    }
}

pub(super) struct Remote(RawSftpSession);

pub(super) async fn start(stream: ChannelStream<Msg>, wait: Duration) -> Result<Remote, Broken> {
    let config = Config {
        max_packet_len: MAX_PACKET,
        request_timeout_secs: wait.as_secs().max(1),
        ..Config::default()
    };
    let sftp = RawSftpSession::new_with_config(stream, config);
    sftp.init().await.map_err(|_| Broken::Sftp)?;
    Ok(Remote(sftp))
}

fn refusal(error: &SftpError) -> Result<Refusal, Broken> {
    match error {
        SftpError::Status(status) => match status.status_code {
            StatusCode::NoSuchFile => Ok(Refusal::NotFound),
            StatusCode::NoConnection | StatusCode::ConnectionLost => Err(Broken::Sftp),
            _ => Ok(Refusal::Denied),
        },
        _ => Err(Broken::Sftp),
    }
}

fn asked<T>(answer: Result<T, SftpError>) -> Asked<T> {
    match answer {
        Ok(value) => Ok(Ok(value)),
        Err(error) => refusal(&error).map(Err),
    }
}

pub(super) fn kind(attrs: &FileAttributes) -> Option<Kind> {
    match attrs.permissions? & TYPE_BITS {
        REGULAR => Some(Kind::File),
        DIRECTORY => Some(Kind::Dir),
        SYMLINK => Some(Kind::Link),
        _ => None,
    }
}

pub(super) fn entry(file: File) -> Option<Entry> {
    let name = file.filename;
    let listed = !matches!(name.as_str(), "" | "." | "..")
        && name.len() <= MAX_NAME_BYTES
        && !name.contains(['/', '\0']);
    let kind = kind(&file.attrs).filter(|_| listed)?;
    let modified_at = file
        .attrs
        .mtime
        .and_then(|seconds| DateTime::from_timestamp(i64::from(seconds), 0));
    Some(Entry {
        name,
        kind,
        size: file.attrs.size,
        modified_at,
    })
}

impl Remote {
    pub(super) async fn list(&self, path: &DevicePath, cap: usize) -> Asked<Listing> {
        let resolved = match asked(self.0.realpath(path.as_str()).await)? {
            Ok(name) => name.files.into_iter().next().map(|file| file.filename),
            Err(refused) => return Ok(Err(refused)),
        };
        let Some(resolved) = resolved.filter(|path| path.len() <= MAX_PATH_BYTES) else {
            return Ok(Err(Refusal::Denied));
        };
        let handle = match asked(self.0.opendir(resolved.as_str()).await)? {
            Ok(handle) => handle.handle,
            Err(refused) => return Ok(Err(refused)),
        };
        let read = self.entries(&handle, cap).await;
        // The listing stands whether or not the device takes the close.
        let _closed = self.0.close(handle).await;
        Ok(read?.map(|entries| Listing {
            path: resolved,
            entries,
        }))
    }

    async fn entries(&self, handle: &str, cap: usize) -> Asked<Vec<Entry>> {
        let mut entries = Vec::new();
        while entries.len() < cap {
            match self.0.readdir(handle).await {
                Ok(name) => entries.extend(name.files.into_iter().filter_map(entry)),
                Err(SftpError::Status(status)) if status.status_code == StatusCode::Eof => break,
                Err(error) => return refusal(&error).map(Err),
            }
        }
        entries.truncate(cap);
        Ok(Ok(entries))
    }

    /// A link is followed.
    pub(super) async fn open_download(&self, path: &DevicePath) -> Asked<(Handle, u64)> {
        match asked(self.0.stat(path.as_str()).await)? {
            Ok(attrs) if kind(&attrs.attrs) == Some(Kind::File) => {}
            Ok(_) => return Ok(Err(Refusal::NotFound)),
            Err(refused) => return Ok(Err(refused)),
        }
        let flags = OpenFlags::READ;
        let handle = match asked(
            self.0
                .open(path.as_str(), flags, FileAttributes::default())
                .await,
        )? {
            Ok(handle) => Handle(handle.handle),
            Err(refused) => return Ok(Err(refused)),
        };
        let size = match asked(self.0.fstat(handle.0.as_str()).await) {
            Ok(Ok(attrs)) => attrs.attrs.size.ok_or(Refusal::Denied),
            Ok(Err(refused)) => Err(refused),
            Err(broken) => return Err(broken),
        };
        if size.is_err() {
            // Refused either way; the device may keep the handle until the session ends.
            let _closed = self.0.close(handle.0.as_str()).await;
        }
        Ok(size.map(|size| (handle, size)))
    }

    /// Exclusive creation: the device itself refuses to replace a file, so the hub never
    /// overwrites.
    pub(super) async fn create(&self, path: &DevicePath) -> Asked<Handle> {
        match self.0.lstat(path.as_str()).await {
            Ok(_) => return Ok(Err(Refusal::Exists)),
            Err(SftpError::Status(status)) if status.status_code == StatusCode::NoSuchFile => {}
            Err(error) => return refusal(&error).map(Err),
        }
        let flags = OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE;
        let error = match self
            .0
            .open(path.as_str(), flags, FileAttributes::default())
            .await
        {
            Ok(handle) => return Ok(Ok(Handle(handle.handle))),
            Err(error) => refusal(&error)?,
        };
        // Something that appeared since the check made the exclusive open fail.
        Ok(Err(match self.0.lstat(path.as_str()).await {
            Ok(_) => Refusal::Exists,
            Err(_) => error,
        }))
    }

    /// Any failure is [`Broken::Sftp`]: mid-transfer, the client has no refusal to read.
    pub(super) async fn read(
        &self,
        handle: &Handle,
        offset: u64,
        len: u32,
    ) -> Result<Vec<u8>, Broken> {
        match self.0.read(handle.0.as_str(), offset, len).await {
            Ok(data) => Ok(data.data),
            Err(SftpError::Status(status)) if status.status_code == StatusCode::Eof => {
                Ok(Vec::new())
            }
            Err(_) => Err(Broken::Sftp),
        }
    }

    pub(super) async fn write(
        &self,
        handle: &Handle,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<(), Broken> {
        self.0
            .write(handle.0.as_str(), offset, data)
            .await
            .map(drop)
            .map_err(|_| Broken::Sftp)
    }

    pub(super) async fn close(&self, handle: &Handle) -> Result<(), Broken> {
        self.0
            .close(handle.0.as_str())
            .await
            .map(drop)
            .map_err(|_| Broken::Sftp)
    }

    pub(super) async fn remove(&self, path: &DevicePath) -> Result<(), Broken> {
        self.0
            .remove(path.as_str())
            .await
            .map(drop)
            .map_err(|_| Broken::Sftp)
    }

    pub(super) async fn alive(&self) -> Result<(), Broken> {
        self.0
            .realpath(".")
            .await
            .map(drop)
            .map_err(|_| Broken::Sftp)
    }
}

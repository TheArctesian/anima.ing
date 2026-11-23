//! The hub's client of the link's control socket. Each call checks the socket before it opens
//! (private path and ACL) and, on Unix, the peer UID after it connects.

use std::{io::ErrorKind, path::PathBuf, time::Duration};

#[cfg(windows)]
use anima_windows::socket::Stream as UnixStream;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
#[cfg(unix)]
use tokio::net::UnixStream;
use zeroize::Zeroizing;

#[cfg(unix)]
use super::owned;
use super::{
    Error,
    wire::{MAX_LINE, Request, parse_reply},
};

/// The link answers from memory; a link that takes longer is stuck.
pub(super) const CALL_LIMIT: Duration = Duration::from_secs(5);

const FIRST_READ: usize = 8 * 1024;

/// The user `uid` (the hub's own) must own and serve the socket.
#[derive(Clone, Debug)]
pub(super) struct Control {
    path: PathBuf,
    #[cfg(unix)]
    uid: u32,
}

impl Control {
    #[cfg(unix)]
    pub fn new(path: PathBuf, uid: u32) -> Self {
        Self { path, uid }
    }

    pub fn of_hub(path: PathBuf) -> Self {
        #[cfg(unix)]
        return Self::new(path, rustix::process::geteuid().as_raw());
        #[cfg(windows)]
        Self { path }
    }

    /// # Errors
    /// An exposed or absent socket, a failed or slow exchange, an unreadable reply, or a refusal.
    pub async fn call<T: DeserializeOwned>(&self, request: Request<'_>) -> Result<T, Error> {
        let line = tokio::time::timeout(CALL_LIMIT, self.exchange(request))
            .await
            .map_err(|_| Error::TimedOut)??;
        parse_reply(&line)
    }

    async fn exchange(&self, request: Request<'_>) -> Result<Zeroizing<Vec<u8>>, Error> {
        #[cfg(unix)]
        {
            let (path, uid) = (self.path.clone(), self.uid);
            // The check reads file metadata and may run `/bin/ls`: off the async workers.
            tokio::task::spawn_blocking(move || owned::check(&path, uid))
                .await
                .map_err(|_| Error::Connection)??;
        }
        let mut stream =
            UnixStream::connect(&self.path)
                .await
                .map_err(|error| match error.kind() {
                    ErrorKind::NotFound | ErrorKind::ConnectionRefused => Error::NotRunning,
                    ErrorKind::PermissionDenied => Error::Exposed("the socket path is not private"),
                    _ => Error::Connection,
                })?;
        #[cfg(unix)]
        peer_is(&stream, self.uid)?;
        stream
            .write_all(&request.line())
            .await
            .map_err(|_| Error::Connection)?;
        read_line(&mut stream).await
    }
}

#[cfg(unix)]
pub(super) fn peer_is(stream: &UnixStream, uid: u32) -> Result<(), Error> {
    let peer = stream.peer_cred().map_err(|_| Error::Connection)?;
    if peer.uid() == uid {
        Ok(())
    } else {
        Err(Error::Exposed("the socket's process runs as another user"))
    }
}

/// Every buffer is zeroed when it drops, so growing it leaves no copy of an invite (LOG-007).
pub(super) async fn read_line(stream: &mut UnixStream) -> Result<Zeroizing<Vec<u8>>, Error> {
    let mut line = Zeroizing::new(vec![0_u8; FIRST_READ]);
    let mut filled = 0;
    let end = loop {
        if filled == line.len() {
            if line.len() > MAX_LINE {
                break filled;
            }
            line = grown(&line, (line.len() * 2).min(MAX_LINE + 1));
        }
        let read = stream
            .read(&mut line[filled..])
            .await
            .map_err(|_| Error::Connection)?;
        let newline = line[filled..filled + read].iter().position(|&b| b == b'\n');
        filled += read;
        match newline {
            Some(at) => break filled - read + at,
            None if read == 0 => break filled,
            None => {}
        }
    };
    line.truncate(end);
    if line.is_empty() || line.len() > MAX_LINE {
        return Err(Error::Unreadable);
    }
    Ok(line)
}

fn grown(old: &[u8], size: usize) -> Zeroizing<Vec<u8>> {
    let mut next = Zeroizing::new(vec![0_u8; size]);
    next[..old.len()].copy_from_slice(old);
    next
}

//! The address `install --listen` gave the hub, which an `update` writes into the unit file again.

use std::fs;
use std::io::{self, Write as _};
use std::net::SocketAddr;

use anima_database::owner_only;

use crate::Folders;

pub(super) fn record(folders: &Folders, listen: Option<SocketAddr>) -> io::Result<()> {
    let file = folders.listen_file();
    let Some(listen) = listen else {
        return match fs::remove_file(&file) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    };
    let mut new = owner_only::new_file(&file)?;
    writeln!(new, "{listen}").and_then(|()| new.sync_all())
}

/// Anything but a loopback socket address counts as none, so the unit has the default (NET-003).
#[must_use]
pub(super) fn recorded(folders: &Folders) -> Option<SocketAddr> {
    let text = crate::plain_file::read_text(&folders.listen_file(), 64).ok()?;
    text.trim()
        .parse::<SocketAddr>()
        .ok()
        .filter(|listen| is_allowed(*listen))
}

/// `listen` when the hub may listen there.
///
/// # Errors
/// [`NotAllowed`] otherwise.
pub fn require_allowed(listen: SocketAddr) -> Result<SocketAddr, NotAllowed> {
    if is_allowed(listen) {
        Ok(listen)
    } else {
        Err(NotAllowed)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("--listen must be a loopback address with a port that is not 0")]
pub struct NotAllowed;

/// A loopback address (NET-003) and a named port, because the unit file keeps it for the hub.
#[must_use]
pub fn is_allowed(listen: SocketAddr) -> bool {
    listen.ip().is_loopback() && listen.port() != 0
}

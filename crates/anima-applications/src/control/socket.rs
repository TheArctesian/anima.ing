//! Own the local listener and identify clients before reading their requests.
use std::{
    fs::{self, File, Permissions},
    io,
    os::unix::{
        fs::{FileTypeExt as _, MetadataExt as _, PermissionsExt as _, chown},
        net::{UnixListener, UnixStream},
    },
    path::Path,
};

use nix::unistd::{Uid, User};
use rustix::{
    fs::{FlockOperation, Mode, OFlags},
    net::sockopt::socket_peercred,
    process::geteuid,
};

pub(super) struct Listener {
    socket: UnixListener,
    _directory: File,
}

impl Listener {
    pub(super) fn bind(directory: &Path, daemon: u32) -> io::Result<Self> {
        let directory_lock = rustix::fs::open(
            directory,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        rustix::fs::flock(&directory_lock, FlockOperation::NonBlockingLockExclusive)?;
        let endpoint = directory.join("control.sock");
        let owner = geteuid().as_raw();
        remove_stale(&endpoint, owner)?;
        let socket = UnixListener::bind(&endpoint)?;
        let mode = if daemon == owner {
            0o600
        } else {
            let user = User::from_uid(Uid::from_raw(daemon))?.ok_or(io::ErrorKind::NotFound)?;
            chown(&endpoint, None, Some(user.gid.as_raw()))?;
            0o660
        };
        fs::set_permissions(&endpoint, Permissions::from_mode(mode))?;
        Ok(Self {
            socket,
            _directory: File::from(directory_lock),
        })
    }

    pub(super) fn accept(&self, daemon: u32) -> io::Result<Option<UnixStream>> {
        let (stream, _) = self.socket.accept()?;
        let accepted = socket_peercred(&stream).is_ok_and(|peer| peer.uid.as_raw() == daemon);
        Ok(accepted.then_some(stream))
    }
}

fn remove_stale(path: &Path, owner: u32) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() && metadata.uid() == owner => {
            fs::remove_file(path)
        }
        Ok(_) => Err(io::ErrorKind::PermissionDenied.into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

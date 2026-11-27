//! `AnimaOS`'s optional root-authenticated ingress inside an isolated workspace.
//! Managed local connections receive agent authority; ordinary host runners are unchanged.
use std::{io, os::unix::fs::PermissionsExt as _, path::Path};
use tokio::net::UnixListener;

pub(super) const PATH: &str = "/run/anima/owner.sock";

pub(in crate::threads::runner) fn bind() -> io::Result<Option<UnixListener>> {
    let Some(path) = std::env::var_os("ANIMA_MANAGED_OWNER_SOCKET") else {
        return Ok(None);
    };
    if path != PATH || !cfg!(target_os = "linux") || rustix::process::getuid().as_raw() != 1000 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "invalid managed runner ingress",
        ));
    }
    let path = Path::new(PATH);
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let listener = UnixListener::bind(path)?;
    // The root broker has no container capabilities. Socket access is permitted
    // here; SO_PEERCRED below admits only uid 0, never the workspace uid.
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o666))?;
    Ok(Some(listener))
}

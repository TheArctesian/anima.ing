use std::{ffi::OsString, os::unix::net::UnixStream};

use core_foundation::{base::TCFType as _, data::CFData};
use nix_peer::sys::socket::{getsockopt, sockopt};
use security_framework::os::macos::code_signing::{
    Flags, GuestAttributes, SecCode, SecRequirement,
};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// The socket is opened inside the child role. An earlier executable holding the bootstrap
/// pipe can read its public address but cannot inherit this newly opened socket.
pub(super) fn signed_child(stream: &UnixStream, child: u32, arguments: &[OsString]) -> Option<()> {
    matches_child(stream, child, arguments, || {
        crate::hub::peer::daemon_requirement(child)?.parse().ok()
    })
}

fn matches_child(
    stream: &UnixStream,
    child: u32,
    arguments: &[OsString],
    requirement: impl FnOnce() -> Option<SecRequirement>,
) -> Option<()> {
    let peer = u32::try_from(getsockopt(stream, sockopt::LocalPeerPid).ok()?).ok()?;
    if peer != child {
        return None;
    }
    let audit = getsockopt(stream, sockopt::LocalPeerToken).ok()?;
    let bytes: Vec<u8> = audit.val.into_iter().flat_map(u32::to_ne_bytes).collect();
    let token = CFData::from_buffer(&bytes);
    let mut attributes = GuestAttributes::new();
    attributes.set_audit_token(token.as_concrete_TypeRef());
    let code = SecCode::copy_guest_with_attribues(None, &attributes, Flags::NONE).ok()?;
    let pid = Pid::from_u32(child);
    let mut system = System::new().ok()?;
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );
    if system.process(pid)?.cmd().get(1..)? != arguments {
        return None;
    }
    code.check_validity(Flags::NONE, &requirement()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bootstrap_connection_belongs_to_its_signed_process_and_role_arguments() {
        let tmp = tempfile::tempdir_in("/tmp").unwrap();
        let address = tmp.path().join("fixture");
        let listener = std::os::unix::net::UnixListener::bind(&address).unwrap();
        let _client = UnixStream::connect(address).unwrap();
        let (stream, _) = listener.accept().unwrap();
        let own = std::process::id();
        let arguments: Vec<_> = std::env::args_os().skip(1).collect();
        let pass = || "always".parse().ok();
        let refuse = || "never".parse().ok();
        assert!(matches_child(&stream, own, &arguments, pass).is_some());
        assert!(matches_child(&stream, own, &arguments, refuse).is_none());
        assert!(matches_child(&stream, own, &["holder-public-root".into()], pass).is_none());
        assert!(matches_child(&stream, 0, &arguments, pass).is_none());
    }
}

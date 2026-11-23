//! The client sends a token or a password to a loopback hub only when a process of this user
//! listens there; for the device link's port, only the app's own `anima-link device` child
//! (F-10, security review M2). The origin is never logged.

mod listeners;
mod origin;

#[cfg(test)]
mod tests;

use std::{
    ffi::OsStr,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    sync::Arc,
    time::Duration,
};

use serde::{Serialize, Serializer, ser::SerializeStruct};
use tauri::{Runtime, State, Webview};

use crate::link::DeviceLink;
use listeners::{Listed, Listener, Owner};

const CONNECT_LIMIT: Duration = Duration::from_millis(300);
const LSOF: &str = "/usr/sbin/lsof";

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HubListenerError {
    #[error("only the main window may check a hub address")]
    Caller,
    #[error("the address is not http://127.0.0.1 with a port")]
    BadOrigin,
}

impl HubListenerError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Caller => "caller",
            Self::BadOrigin => "bad-origin",
        }
    }
}

/// Serialises as `{ "code": ..., "message": ... }` with the fixed `Display` text.
impl Serialize for HubListenerError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("HubListenerError", 2)?;
        out.serialize_field("code", self.code())?;
        out.serialize_field("message", &self.to_string())?;
        out.end()
    }
}

fn from_main(label: &str) -> Result<(), HubListenerError> {
    crate::web_view::caller(label).map_err(|_| HubListenerError::Caller)
}

fn system_listeners(port: u16, uid: u32) -> (Vec<Listener>, Listed) {
    if cfg!(target_os = "macos") {
        (lsof_listeners(port, uid), Listed::ThisUser)
    } else {
        (proc_listeners(), Listed::EveryUser)
    }
}

fn lsof_listeners(port: u16, uid: u32) -> Vec<Listener> {
    use crate::os_program::{self, AfterLimit, Limit, Stdout};
    let (uid, port) = (uid.to_string(), format!("-iTCP:{port}"));
    let args: [&OsStr; 7] =
        ["-nP", "-a", "-u", &uid, &port, "-sTCP:LISTEN", "-Fnt"].map(OsStr::new);
    let limit = Limit {
        patience: Duration::from_secs(3),
        after: AfterLimit::Kill,
    };
    // `lsof` exits with a failure when it finds nothing.
    os_program::run(
        LSOF,
        &args,
        limit,
        Stdout::Capture {
            max_bytes: 64 * 1024,
        },
    )
    .map_or_default(|printed| listeners::lsof_names(&String::from_utf8_lossy(&printed)))
}

fn proc_listeners() -> Vec<Listener> {
    let read = |path, ipv6| {
        std::fs::read_to_string(path).map_or_default(|text| listeners::proc_net_tcp(&text, ipv6))
    };
    let mut all = read("/proc/net/tcp", false);
    all.extend(read("/proc/net/tcp6", true));
    all
}

fn owner(port: u16, link_permits: bool) -> Owner {
    let uid = nix::unistd::getuid().as_raw();
    let (found, listed) = system_listeners(port, uid);
    let target = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let accepts = TcpStream::connect_timeout(&SocketAddr::new(target, port), CONNECT_LIMIT).is_ok();
    let own = link_permits && listeners::only_own(&found, listed, (target, port), uid);
    listeners::verdict(accepts, own)
}

/// `"own"`, `"other"` or `"none"` for `http://127.0.0.1:<port>`. The client sends nothing to
/// an address that is not `"own"`.
#[tauri::command]
pub async fn hub_listener_check<R: Runtime>(
    webview: Webview<R>,
    device: State<'_, Arc<DeviceLink>>,
    origin: String,
) -> Result<Owner, HubListenerError> {
    from_main(webview.label())?;
    let port = origin::port(&origin).ok_or(HubListenerError::BadOrigin)?;
    let link_permits = listeners::link_permits(&device.state(), port);
    Ok(
        tauri::async_runtime::spawn_blocking(move || owner(port, link_permits))
            .await
            .unwrap_or(Owner::Other),
    )
}

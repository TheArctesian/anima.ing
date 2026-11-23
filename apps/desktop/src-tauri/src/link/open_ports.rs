//! The open ports of this computer (HMD-049): the local ports that its hub may reach through
//! the link (HMD-048). The list is a file of the link program in the device folder;
//! `anima-link ports` shows it and turns one port on or off. Only the person changes it, here or
//! in a terminal; the hub cannot. The link reads the file at each request of the hub, so a
//! change needs no restart of the link or of its unit.

use std::{ffi::OsString, path::Path, time::Duration};

use serde::{Deserialize, Serialize};
use tauri::{Runtime, Webview};

use super::{LinkError, PROGRAM, environment, folders, from_main};

/// How long `anima-link ports` may take: it reads or writes one small file.
const PORTS_LIMIT: Duration = Duration::from_secs(5);

/// The ports that the link program lets a person turn on: SSH and screen sharing, and remote
/// desktop on Windows and Linux. The link program checks the same list again.
pub fn may_open(port: u16, rdp: bool) -> bool {
    matches!(port, 22 | 5900) || (port == 3389 && rdp)
}

/// What `anima-link ports` answers and the shell gives the client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenPorts {
    pub open: Vec<u16>,
}

/// The answer of `anima-link ports`, `{ "open": [22, 5900] }`; none for anything else, and for
/// a port that the link may not open.
pub fn open_ports(printed: &[u8], rdp: bool) -> Option<OpenPorts> {
    let ports: OpenPorts = serde_json::from_slice(printed).ok()?;
    ports
        .open
        .iter()
        .all(|port| may_open(*port, rdp))
        .then_some(ports)
}

/// The arguments of `anima-link ports` on `folder`, with the change when there is one.
pub fn arguments(folder: &Path, change: Option<(u16, bool)>) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("ports"),
        OsString::from("-state"),
        folder.as_os_str().to_owned(),
    ];
    if let Some((port, on)) = change {
        args.push(OsString::from(if on { "on" } else { "off" }));
        args.push(OsString::from(port.to_string()));
    }
    args
}

/// Runs `anima-link ports` of `program` on `folder`.
pub fn ask(
    program: &Path,
    folder: &Path,
    change: Option<(u16, bool)>,
) -> Result<OpenPorts, LinkError> {
    let rdp = cfg!(any(target_os = "windows", target_os = "linux"));
    if change.is_some_and(|(port, _)| !may_open(port, rdp)) {
        return Err(LinkError::Failed);
    }
    let args = arguments(folder, change);
    let env = environment(|name| std::env::var_os(name));
    let invocation = crate::carried::run::Invocation {
        program,
        args: &args.iter().collect::<Vec<_>>(),
        env: &env,
        limit: PORTS_LIMIT,
    };
    let printed = crate::carried::run::run(&invocation).map_err(|_| LinkError::Failed)?;
    open_ports(&printed, rdp).ok_or(LinkError::Failed)
}

/// The open ports of this computer, after the change `port` to `on` when both are given. A
/// device that has not joined a hub has no list.
#[tauri::command]
pub async fn link_ports<R: Runtime>(
    webview: Webview<R>,
    port: Option<u16>,
    on: Option<bool>,
) -> Result<OpenPorts, LinkError> {
    from_main(webview.label())?;
    let change = port.zip(on);
    let program = crate::carried::program(PROGRAM).ok_or(LinkError::NotCarried)?;
    let folders = folders()?;
    if !folders.device.is_dir() {
        return Err(LinkError::Folder);
    }
    if change == Some((22, true)) {
        remote_login::enable(remote_login::Prompt::Desktop)
            .await
            .map_err(|_| LinkError::SshSetup)?;
    }
    tauri::async_runtime::spawn_blocking(move || ask(&program, &folders.device, change))
        .await
        .unwrap_or(Err(LinkError::Failed))
}

//! Only the person changes the open ports, here or in a terminal; the hub cannot (HMD-048,
//! HMD-049).
//! The link reads the file at each request of the hub, so a change needs no restart.

use std::{ffi::OsString, path::Path, time::Duration};

use serde::{Deserialize, Serialize};
use tauri::{Runtime, State, Webview};

use super::{LinkError, PROGRAM, environment, folders, from_main};

const PORTS_LIMIT: Duration = Duration::from_secs(5);

/// The link program checks the same list again.
pub fn may_open(port: u16, rdp: bool) -> bool {
    matches!(port, 22 | 5900) || (port == 3389 && rdp)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenPorts {
    pub open: Vec<u16>,
}

pub fn open_ports(printed: &[u8], rdp: bool) -> Option<OpenPorts> {
    let ports: OpenPorts = serde_json::from_slice(printed).ok()?;
    ports
        .open
        .iter()
        .all(|port| may_open(*port, rdp))
        .then_some(ports)
}

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

/// A device that has not joined a hub has no list.
#[tauri::command]
pub async fn link_ports<R: Runtime>(
    webview: Webview<R>,
    changing: State<'_, super::Changing>,
    port: Option<u16>,
    on: Option<bool>,
) -> Result<OpenPorts, LinkError> {
    from_main(webview.label())?;
    let _claim = changing.claim()?;
    let change = port.zip(on);
    if change.is_some_and(|(port, _)| {
        !may_open(port, cfg!(any(target_os = "windows", target_os = "linux")))
    }) {
        return Err(LinkError::Failed);
    }
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
    if let Some((port @ (3389 | 5900), true)) = change {
        remote_login::desktop::enable(port, remote_login::Prompt::Desktop)
            .await
            .map_err(|_| LinkError::DesktopSetup)?;
    }
    if let Some((port, on)) = change {
        super::access::remember(&folders.device, port, on)?;
    }
    tauri::async_runtime::spawn_blocking(move || ask(&program, &folders.device, change))
        .await
        .unwrap_or(Err(LinkError::Failed))
}

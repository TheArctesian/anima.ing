//! First-run choices are persisted before preparation, so failed or cancelled setup is retryable.

use super::{Changing, LinkError, PROGRAM, folders, from_main, open_ports};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use tauri::{AppHandle, Manager as _, Runtime, State, Webview};

const CHOICES: &str = "remote-access-choices";

#[derive(Serialize)]
pub struct Access {
    configured: bool,
    desktop: bool,
    #[serde(rename = "macRdp")]
    mac_rdp: bool,
    username: String,
    selected: Vec<u16>,
    open: Vec<u16>,
    ready: Vec<u16>,
    listening: Vec<u16>,
    notes: Vec<Note>,
}

#[derive(Serialize)]
struct Note {
    port: u16,
    message: String,
}

#[derive(Serialize, Deserialize)]
struct Choices {
    ports: Vec<u16>,
}

fn saved(folder: &Path) -> Result<Option<Vec<u16>>, LinkError> {
    match fs::read(folder.join(CHOICES)) {
        Ok(bytes) => serde_json::from_slice::<Choices>(&bytes)
            .map(|choices| Some(choices.ports))
            .map_err(|_| LinkError::Folder),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(LinkError::Folder),
    }
}

fn defaults(desktop: bool, rdp: bool) -> Vec<u16> {
    let mut ports = vec![22];
    if desktop {
        if rdp {
            ports.push(3389);
        }
        ports.push(5900);
    }
    ports
}

pub(super) async fn prepare<R: Runtime>(
    app: &AppHandle<R>,
    folder: &Path,
    port: u16,
) -> Result<(), String> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    if port == 3389 {
        return super::rdp::prepare(app, folder)
            .await
            .map_err(str::to_owned);
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    let _ = (app, folder);
    if port == 22 {
        return remote_login::enable(remote_login::Prompt::Desktop)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string());
    }
    remote_login::desktop::enable(port, remote_login::Prompt::Desktop).await.map_err(|error| {
        if matches!(error, remote_login::Error::VncAuthentication) {
            error.to_string()
        } else if matches!(error, remote_login::Error::Unsupported) {
            match (std::env::consts::OS, port) {
                ("macos", 5900) => "Turn on Screen Sharing in System Settings → General → Sharing, then try again.",
                ("linux", 5900) => "Configure a VNC server with mandatory protocol authentication, then try again.",
                ("windows", 5900) => "Install and configure an authenticated VNC server, then try again. Windows has no built-in VNC server.",
                _ => "This system cannot prepare this protocol automatically. Configure its sharing server, then try again.",
            }.to_owned()
        } else { "Desktop sharing setup failed, permission was cancelled, or the server did not become ready. Check the system prompt and try again.".to_owned() }
    })
}

fn store(folder: &Path, ports: Vec<u16>) -> Result<(), LinkError> {
    let bytes = serde_json::to_vec(&Choices { ports }).map_err(|_| LinkError::Folder)?;
    fs::write(folder.join(CHOICES), bytes).map_err(|_| LinkError::Folder)
}

pub(super) fn remember(folder: &Path, port: u16, on: bool) -> Result<(), LinkError> {
    if let Some(mut ports) = saved(folder)? {
        ports.retain(|value| *value != port);
        if on {
            ports.push(port);
        }
        store(folder, ports)?;
    }
    Ok(())
}

fn changes(selected: &[u16], target: Option<u16>, rdp: bool) -> Vec<(u16, bool)> {
    let mut ports: Vec<_> = [22, 3389, 5900]
        .into_iter()
        .filter(|port| {
            open_ports::may_open(*port, rdp) && target.is_none_or(|value| value == *port)
        })
        .map(|port| (port, selected.contains(&port)))
        .collect();
    ports.sort_by_key(|(_, on)| *on);
    ports
}

async fn apply<R: Runtime>(
    app: &AppHandle<R>,
    paths: (&Path, &Path),
    selected: &[u16],
    target: Option<u16>,
) -> Result<Vec<Note>, LinkError> {
    let (folder, program) = paths;
    store(folder, selected.to_vec())?;
    let mut notes = Vec::new();
    for (port, on) in changes(selected, target, open_ports::RDP_SUPPORTED) {
        if !on || (port == 22 && remote_login::ready().await) {
            #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
            if port == 3389 {
                super::rdp::close();
            }
            open_ports::ask(program, folder, Some((port, on)))?;
            continue;
        }
        match prepare(app, folder, port).await {
            Ok(()) => {
                open_ports::ask(program, folder, Some((port, true)))?;
            }
            Err(message) => {
                open_ports::ask(program, folder, Some((port, false)))?;
                notes.push(Note { port, message });
            }
        }
    }
    Ok(notes)
}

async fn readiness(rdp: bool) -> (Vec<u16>, Vec<u16>) {
    use remote_login::desktop::Status;
    let mut ready = Vec::new();
    let mut listening = Vec::new();
    if remote_login::ready().await {
        ready.push(22);
    }
    for port in [3389, 5900] {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        if port == 3389 {
            if super::rdp::running() {
                listening.push(port);
                ready.push(port);
            }
            continue;
        }
        if open_ports::may_open(port, rdp) {
            match remote_login::desktop::status(port).await {
                Status::Unavailable => {}
                Status::Listening => listening.push(port),
                Status::Ready => {
                    listening.push(port);
                    ready.push(port);
                }
            }
        }
    }
    (ready, listening)
}

/// Reads the setup choices, or applies the person's selections. It never runs on app startup.
#[tauri::command]
pub async fn link_access<R: Runtime>(
    webview: Webview<R>,
    changing: State<'_, Changing>,
    ports: Option<Vec<u16>>,
    target: Option<u16>,
) -> Result<Access, LinkError> {
    from_main(webview.label())?;
    let _claim = changing.claim()?;
    let folder = folders()?.device;
    if !folder.is_dir() {
        return Err(LinkError::Folder);
    }
    let program = crate::carried::program(PROGRAM).ok_or(LinkError::NotCarried)?;
    let rdp = open_ports::RDP_SUPPORTED;
    let desktop = remote_login::desktop::present();
    let open = open_ports::ask(&program, &folder, None)?.open;
    let before = saved(&folder)?;
    let configured = before.is_some() || folder.join("open-ports").exists();
    let mut selected = before.unwrap_or_else(|| {
        if configured {
            open.clone()
        } else {
            defaults(desktop, rdp && !open_ports::MAC_RDP)
        }
    });
    let mut notes = Vec::new();
    let applying = ports.is_some();
    if target.is_some_and(|port| !open_ports::may_open(port, rdp))
        || (target.is_some() && !applying)
    {
        return Err(LinkError::Failed);
    }
    if let Some(mut ports) = ports {
        if ports
            .iter()
            .any(|port| !open_ports::may_open(*port, rdp) || (*port != 22 && !desktop))
        {
            return Err(LinkError::Failed);
        }
        ports.sort_unstable();
        ports.dedup();
        selected = ports;
        notes = apply(webview.app_handle(), (&folder, &program), &selected, target).await?;
    }
    let (ready, listening) = readiness(rdp).await;
    Ok(Access {
        configured: configured || applying,
        desktop,
        mac_rdp: open_ports::MAC_RDP,
        username: whoami::username().map_err(|_| LinkError::Failed)?,
        selected,
        open: open_ports::ask(&program, &folder, None)?.open,
        notes,
        ready,
        listening,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_capability_action_never_changes_a_sibling_port() {
        assert_eq!(changes(&[22, 5900], Some(22), false), [(22, true)]);
        assert_eq!(changes(&[5900], Some(5900), false), [(5900, true)]);
        assert_eq!(changes(&[5900], Some(22), false), [(22, false)]);
        assert_eq!(changes(&[22], None, false), [(5900, false), (22, true)]);
    }

    #[test]
    fn new_machines_offer_only_applicable_protocols() {
        assert_eq!(defaults(false, true), [22]);
        assert_eq!(defaults(true, false), [22, 5900]);
        assert_eq!(defaults(true, true), [22, 3389, 5900]);
    }

    #[test]
    fn saved_opt_out_is_distinct_from_a_machine_without_choices() {
        let folder = tempfile::tempdir().unwrap();
        assert_eq!(saved(folder.path()).unwrap(), None);
        fs::write(folder.path().join(CHOICES), br#"{"ports":[]}"#).unwrap();
        assert_eq!(saved(folder.path()).unwrap(), Some(vec![]));
        store(folder.path(), vec![22, 5900]).unwrap();
        remember(folder.path(), 5900, false).unwrap();
        assert_eq!(saved(folder.path()).unwrap(), Some(vec![22]));
        remember(folder.path(), 22, false).unwrap();
        assert_eq!(saved(folder.path()).unwrap(), Some(vec![]));
        fs::write(folder.path().join(CHOICES), b"damaged").unwrap();
        assert!(saved(folder.path()).is_err());
    }
}

//! First-run choices are persisted before preparation, so failed or cancelled setup is retryable.

use super::{Changing, LinkError, PROGRAM, folders, from_main, open_ports};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use tauri::{Runtime, State, Webview};

const CHOICES: &str = "remote-access-choices";

#[derive(Serialize)]
pub struct Access {
    configured: bool,
    desktop: bool,
    username: String,
    selected: Vec<u16>,
    open: Vec<u16>,
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

async fn prepare(port: u16) -> Result<(), String> {
    if port == 22 {
        return remote_login::enable(remote_login::Prompt::Desktop)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string());
    }
    remote_login::desktop::enable(port, remote_login::Prompt::Desktop).await.map_err(|error| {
        if matches!(error, remote_login::Error::Unsupported) {
            match (std::env::consts::OS, port) {
                ("macos", 5900) => "Turn on Screen Sharing in System Settings → General → Sharing, then Retry.",
                ("linux", 5900) => "Enable screen sharing in your desktop settings, then Retry. Automatic VNC setup needs an X11 desktop on display :0.",
                ("windows", 5900) => "Install and configure an authenticated VNC server, then Retry. Windows has no built-in VNC server.",
                _ => "This system cannot prepare this protocol automatically. Configure its sharing server, then Retry.",
            }.to_owned()
        } else { "Desktop sharing setup failed, permission was cancelled, or the server did not become ready. Check the system prompt and Retry.".to_owned() }
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

async fn apply(folder: &Path, program: &Path, selected: &[u16]) -> Result<Vec<Note>, LinkError> {
    store(folder, selected.to_vec())?;
    let rdp = cfg!(any(target_os = "linux", target_os = "windows"));
    for port in [22, 3389, 5900]
        .into_iter()
        .filter(|port| open_ports::may_open(*port, rdp))
    {
        if !selected.contains(&port) {
            open_ports::ask(program, folder, Some((port, false)))?;
        }
    }
    let mut notes = Vec::new();
    for port in selected {
        if *port == 22 && remote_login::ready().await {
            open_ports::ask(program, folder, Some((*port, true)))?;
            continue;
        }
        match prepare(*port).await {
            Ok(()) => {
                open_ports::ask(program, folder, Some((*port, true)))?;
            }
            Err(message) => {
                open_ports::ask(program, folder, Some((*port, false)))?;
                notes.push(Note {
                    port: *port,
                    message,
                });
            }
        }
    }
    Ok(notes)
}

/// Reads the setup choices, or applies the person's selections. It never runs on app startup.
#[tauri::command]
pub async fn link_access<R: Runtime>(
    webview: Webview<R>,
    changing: State<'_, Changing>,
    ports: Option<Vec<u16>>,
) -> Result<Access, LinkError> {
    from_main(webview.label())?;
    let _claim = changing.claim()?;
    let folder = folders()?.device;
    if !folder.is_dir() {
        return Err(LinkError::Folder);
    }
    let program = crate::carried::program(PROGRAM).ok_or(LinkError::NotCarried)?;
    let rdp = cfg!(any(target_os = "linux", target_os = "windows"));
    let desktop = remote_login::desktop::present();
    let open = open_ports::ask(&program, &folder, None)?.open;
    let before = saved(&folder)?;
    let configured = before.is_some() || folder.join("open-ports").exists();
    let mut selected = before.unwrap_or_else(|| {
        if configured {
            open.clone()
        } else {
            defaults(desktop, rdp)
        }
    });
    let mut notes = Vec::new();
    let applying = ports.is_some();
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
        notes = apply(&folder, &program, &selected).await?;
    }
    Ok(Access {
        configured: configured || applying,
        desktop,
        username: whoami::username().map_err(|_| LinkError::Failed)?,
        selected,
        open: open_ports::ask(&program, &folder, None)?.open,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

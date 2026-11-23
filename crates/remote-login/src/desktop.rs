//! Local desktop discovery and preparation. No hub can invoke these operations.

use std::{path::Path, time::Duration};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use crate::{Error, Prompt};

/// Whether this machine has a desktop, including an installed but logged-out Linux desktop.
#[must_use]
pub fn present() -> bool {
    if cfg!(target_os = "macos") {
        return true;
    }
    if cfg!(target_os = "windows") {
        return std::env::var_os("SystemRoot")
            .is_some_and(|root| Path::new(&root).join("explorer.exe").is_file());
    }
    ["/usr/share/xsessions", "/usr/share/wayland-sessions"]
        .iter()
        .any(|folder| {
            std::fs::read_dir(folder).is_ok_and(|mut entries| {
                entries.any(|entry| {
                    entry.is_ok_and(|entry| {
                        entry.path().extension().is_some_and(|ext| ext == "desktop")
                    })
                })
            })
        })
}

/// Checks a protocol reply, rather than treating an unrelated listener as a sharing server.
pub async fn ready(port: u16) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut stream =
            tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).await?;
        if port == 5900 {
            return Ok::<_, std::io::Error>(crate::vnc::requires_authentication(&mut stream).await);
        }
        if port != 3389 {
            return Ok(false);
        }
        stream
            .write_all(&[3, 0, 0, 19, 14, 224, 0, 0, 0, 0, 0, 1, 0, 8, 0, 3, 0, 0, 0])
            .await?;
        let mut reply = [0; 11];
        stream.read_exact(&mut reply).await?;
        Ok(reply[0] == 3 && reply[1] == 0 && reply[5] == 0xd0)
    })
    .await
    .is_ok_and(|result| matches!(result, Ok(true)))
}

/// Prepares supported sharing servers. Unsupported systems require their own sharing settings.
///
/// # Errors
/// Returns an error when permission is denied, no desktop exists, or no verified server starts.
pub async fn enable(port: u16, prompt: Prompt) -> Result<(), Error> {
    if !present() {
        return Err(Error::Unsupported);
    }
    if ready(port).await {
        return Ok(());
    }
    if port == 5900 {
        return Err(Error::VncAuthentication);
    }
    let user = whoami::username().map_err(|_| Error::User)?;
    if !super::valid_user(&user) {
        return Err(Error::User);
    }
    let source = source(port).ok_or(Error::Unsupported)?;
    crate::elevation::run_source(prompt, &source.replace("ANIMA_LOGIN_USER", &user)).await?;
    for _ in 0..15 {
        if ready(port).await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(Error::NotReady)
}

fn source(port: u16) -> Option<&'static str> {
    match (std::env::consts::OS, port) {
        ("linux", 3389) => Some(include_str!("rdp-linux.sh")),
        ("windows", 3389) => Some(include_str!("rdp-windows.ps1")),
        _ => None,
    }
}

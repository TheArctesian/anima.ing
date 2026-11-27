//! Installed service hints are distinct from authenticated protocol readiness.
#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;

use daemon_core::inventory::{Inventory, RemoteAccess, is_remote_user};
use remote_login::{LocalProtocol, Observation, observe};
use serde::{Deserialize, de::DeserializeOwned};
use tokio::io::AsyncReadExt as _;

const SETTINGS: &str = "/etc/anima/remote-access.json";
const PORTS: &str = "/var/lib/anima-link/open-ports";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Installed {
    #[serde(rename = "sshUser")]
    ssh: Option<String>,
    #[serde(rename = "rdpUser")]
    rdp: Option<String>,
    #[serde(rename = "vncUser")]
    vnc: Option<String>,
}

pub(crate) async fn refresh(inventory: &mut Inventory) {
    (inventory.remote_access, inventory.remote_vnc_user) = read().await;
}

async fn read() -> (Option<RemoteAccess>, Option<String>) {
    let Some(installed) = installed().await else {
        return (None, None);
    };
    let ports = selected_ports().await;
    let (ssh, rdp, vnc) = tokio::join!(
        hint(&ports, LocalProtocol::Ssh, installed.ssh),
        hint(&ports, LocalProtocol::Rdp, installed.rdp),
        hint(&ports, LocalProtocol::Vnc, installed.vnc),
    );
    // A local owner may close a port while its protocol negotiation is in progress.
    let ports = selected_ports().await;
    (
        Some(RemoteAccess {
            ssh_user: ssh.filter(|_| ports.contains(&LocalProtocol::Ssh.port())),
            rdp_user: rdp.filter(|_| ports.contains(&LocalProtocol::Rdp.port())),
        }),
        vnc.filter(|_| ports.contains(&LocalProtocol::Vnc.port())),
    )
}

async fn hint(ports: &[u16], protocol: LocalProtocol, user: Option<String>) -> Option<String> {
    let user = user?;
    if ports.contains(&protocol.port()) && observe(protocol).await == Observation::Responding {
        Some(user)
    } else {
        None
    }
}

async fn selected_ports() -> Vec<u16> {
    read_json(PORTS, 16 * 1024).await.unwrap_or_default()
}

async fn installed() -> Option<Installed> {
    let installed: Installed = read_json(SETTINGS, 64 * 1024).await?;
    if [&installed.ssh, &installed.rdp, &installed.vnc]
        .into_iter()
        .flatten()
        .any(|user| !is_remote_user(user))
    {
        return None;
    }
    Some(installed)
}

async fn read_json<T: DeserializeOwned>(path: &str, limit: usize) -> Option<T> {
    let mut options = tokio::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use rustix::fs::OFlags;

        let mut flags = OFlags::NONBLOCK | OFlags::NOCTTY;
        if path == PORTS {
            flags |= OFlags::NOFOLLOW;
        }
        options.custom_flags(flags.bits().cast_signed());
    }
    let file = options.open(path).await.ok()?;
    let metadata = file.metadata().await.ok()?;
    if !metadata.is_file() {
        return None;
    }
    #[cfg(unix)]
    if path == PORTS
        && (metadata.mode() & 0o077 != 0 || metadata.uid() != rustix::process::getuid().as_raw())
    {
        return None;
    }
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .ok()?;
    if bytes.len() > limit {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

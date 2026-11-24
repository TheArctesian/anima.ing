//! This computer for the Add device form (R-058, R-060). The web view passes in only `vpnLoginServer`.
//! None of it is secret, but the user name and the address are not logged (LOG-007).

mod address;
mod name;
mod vpn_client;

#[cfg(all(test, unix))]
mod tests;

use std::{net::Ipv4Addr, time::Duration};

use serde::{Serialize, Serializer, ser::SerializeStruct};
use tauri_plugin_log::log;

const SSH_PATIENCE: Duration = Duration::from_millis(300);
const SSH_PORT: u16 = 22;

/// K1 `OperatingSystem`, for the systems the desktop app runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum OperatingSystem {
    Macos,
    Linux,
    Windows,
}

impl OperatingSystem {
    pub(crate) fn current() -> Option<Self> {
        if cfg!(target_os = "macos") {
            Some(Self::Macos)
        } else if cfg!(target_os = "linux") {
            Some(Self::Linux)
        } else if cfg!(target_os = "windows") {
            Some(Self::Windows)
        } else {
            None
        }
    }
}

pub(crate) fn computer_name() -> String {
    OperatingSystem::current().map_or_default(name::computer_name)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThisMachine {
    pub name: String,
    pub user: String,
    pub os: OperatingSystem,
    /// As the Tailscale client logged in to the hub's login server reports it (ADR-005); else `None`.
    pub vpn_address: Option<Ipv4Addr>,
    pub ssh_listening: bool,
}

/// Carries no OS error text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ThisMachineError {
    #[error("this computer cannot be read on this system")]
    Unavailable,
    #[error("this computer could not be read")]
    Failed,
}

impl ThisMachineError {
    /// Stable code the client switches on.
    fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Failed => "failed",
        }
    }
}

/// Serialises as `{ "code": ..., "message": ... }` with the fixed `Display` text.
impl Serialize for ThisMachineError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("ThisMachineError", 2)?;
        out.serialize_field("code", self.code())?;
        out.serialize_field("message", &self.to_string())?;
        out.end()
    }
}

/// Blocks for up to the name, VPN client and SSH patience.
fn read(os: OperatingSystem, login_server: &str) -> ThisMachine {
    let vpn_address = vpn_client::client_program(os).and_then(|program| {
        vpn_client::own_address(program, login_server, vpn_client::CLIENT_PATIENCE)
    });
    let ssh_listening = address::ssh_answers(vpn_address, SSH_PORT, SSH_PATIENCE);
    ThisMachine {
        name: name::computer_name(os),
        user: whoami::username().unwrap_or_default(),
        os,
        vpn_address,
        ssh_listening,
    }
}

/// `login_server` is the hub's `vpnLoginServer`: only a client logged in to it gives the VPN address.
#[tauri::command]
pub async fn this_machine_read(login_server: String) -> Result<ThisMachine, ThisMachineError> {
    let Some(os) = OperatingSystem::current() else {
        return Err(ThisMachineError::Unavailable);
    };
    let machine = tauri::async_runtime::spawn_blocking(move || read(os, &login_server))
        .await
        .map_err(|_| ThisMachineError::Failed);
    if let Err(error) = &machine {
        log::warn!("this computer could not be read: {}", error.code());
    }
    machine
}

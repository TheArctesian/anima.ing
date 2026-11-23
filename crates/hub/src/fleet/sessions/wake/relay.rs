//! The wake relay runs one command only: `wakeonlan` with the MAC address the hub renders (D-4,
//! CHK-012).

use std::time::Duration;

use super::Error;
use crate::fleet::{
    devices::{Device, MacAddress},
    ssh::{FixedCommand, SshLink},
};

const PROGRAM: &str = "wakeonlan";
/// `wakeonlan` sends one UDP broadcast; ten seconds leaves room for a slow relay.
pub(super) const LIMIT: Duration = Duration::from_secs(10);

/// A device that is off cannot wake itself, so it is never its own relay.
pub(super) fn chosen(device: &Device, relay: Option<Device>) -> Result<Device, Error> {
    let relay = relay.ok_or(Error::NoRelay)?;
    if relay.id == device.id {
        return Err(Error::RelayIsDevice);
    }
    Ok(relay)
}

/// The output is dropped unread (LOG-007).
pub(super) async fn send(link: &mut SshLink, mac: MacAddress) -> Result<(), Error> {
    let command = FixedCommand::new(PROGRAM).mac(mac);
    let output = link.run(&command, LIMIT).await.map_err(Error::Link)?;
    if output.exit_status == Some(0) {
        Ok(())
    } else {
        Err(Error::NotSent)
    }
}

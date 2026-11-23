//! Which devices of the link are connected now (HMD-047).

use std::{net::IpAddr, path::Path};

use chrono::{DateTime, Utc};

use super::{model::ListedDevice, socket::Control, wire::Request};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Presence {
    /// The host of its device in the hub.
    pub address: IpAddr,
    /// `None` when the device did not connect since the link started, or its link program
    /// does not report it.
    pub seen: Option<DateTime<Utc>>,
}

/// # Errors
/// A fixed fault word for the log (LOG-007).
pub async fn presence(control: &Path) -> Result<Vec<Presence>, &'static str> {
    let listed: Vec<ListedDevice> = Control::of_hub(control.to_owned())
        .call(Request::DevicesList)
        .await
        .map_err(|error| error.fault())?;
    let presence = listed.into_iter().map(|device| Presence {
        address: device.address,
        seen: device.seen,
    });
    Ok(presence.collect())
}

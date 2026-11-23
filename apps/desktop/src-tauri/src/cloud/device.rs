use serde::Serialize;

use super::name;
use crate::this_machine::OperatingSystem;

/// HMD-044: lets the client find this computer's link device. None of it is secret.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct CloudDevice {
    /// What `cloud_hub_access` sent.
    name: String,
    computer: String,
    os: OperatingSystem,
}

pub(super) fn cloud_device(computer: &str, os: OperatingSystem) -> CloudDevice {
    CloudDevice {
        name: name::device_name(computer),
        computer: computer.to_owned(),
        os,
    }
}

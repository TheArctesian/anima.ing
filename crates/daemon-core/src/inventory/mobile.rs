//! Installed mobile tooling (THR-172, K8 Section 10), never device instances.

mod parse;
mod probe;
#[cfg(test)]
mod tests;
mod validation;

pub use parse::{device_types, emulator, sdk_packages, simulator_runtimes};
pub use probe::{Entries, Probe, State, Version};

use serde::{Deserialize, Serialize};

/// Fixed read-only catalogue probes (CAP-021, CAP-028).
pub const SIMULATOR_RUNTIMES: super::Program = super::Program {
    name: "xcrun",
    args: &["simctl", "list", "runtimes", "-j"],
};
pub const DEVICE_TYPES: super::Program = super::Program {
    name: "xcrun",
    args: &["simctl", "list", "devicetypes", "-j"],
};
pub const EMULATOR: super::Program = super::Program {
    name: "emulator",
    args: &["-version"],
};

pub const INPUT_LIMIT: usize = 1024 * 1024;
pub const WIRE_LIMIT: usize = 256 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileTools {
    pub apple: Apple,
    pub android: Android,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Apple {
    pub simulator_runtimes: Probe<Entries<SimulatorRuntime>>,
    pub device_types: Probe<Entries<DeviceType>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Android {
    pub sdk_packages: Probe<Entries<SdkPackage>>,
    pub emulator: Probe<Version>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulatorRuntime {
    pub identifier: String,
    pub version: String,
    pub available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceType {
    pub identifier: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkPackage {
    pub path: String,
    pub revision: String,
}

impl MobileTools {
    #[must_use]
    pub const fn unavailable(state: State) -> Self {
        Self {
            apple: Apple {
                simulator_runtimes: Probe::Unavailable(state),
                device_types: Probe::Unavailable(state),
            },
            android: Android {
                sdk_packages: Probe::Unavailable(state),
                emulator: Probe::Unavailable(state),
            },
        }
    }

    #[must_use]
    pub fn accepted(&self) -> bool {
        self.apple.simulator_runtimes.valid(128, |entry| {
            (
                entry.identifier.as_str(),
                validation::identifier(&entry.identifier) && super::is_version(&entry.version),
            )
        }) && self.apple.device_types.valid(256, |entry| {
            (
                entry.identifier.as_str(),
                validation::identifier(&entry.identifier),
            )
        }) && self.android.sdk_packages.valid(512, |entry| {
            (
                entry.path.as_str(),
                validation::package(&entry.path) && super::is_version(&entry.revision),
            )
        }) && match &self.android.emulator {
            Probe::Read(version) => super::is_version(&version.version),
            Probe::Unavailable(_) => true,
        } && serde_json::to_vec(self).is_ok_and(|bytes| bytes.len() <= WIRE_LIMIT)
    }
}

impl<'de> Deserialize<'de> for MobileTools {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        #[derive(Deserialize)]
        struct Snapshot {
            apple: Apple,
            android: Android,
        }
        let value = serde_json::Value::deserialize(deserializer)?;
        if serde_json::to_vec(&value).map_err(D::Error::custom)?.len() > WIRE_LIMIT {
            return Err(D::Error::custom("mobile inventory exceeds wire limit"));
        }
        let snapshot: Snapshot = serde_json::from_value(value).map_err(D::Error::custom)?;
        Ok(Self {
            apple: snapshot.apple,
            android: snapshot.android,
        })
    }
}

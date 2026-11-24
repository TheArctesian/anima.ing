//! The accepted THR-172 read surface; inventory is not device readiness.
#![expect(
    clippy::unused_async_trait_impl,
    reason = "async-graphql Enum derive generates async trait impls"
)]

use async_graphql::{Enum, SimpleObject};
use chrono::{DateTime, Utc};
use daemon_core::inventory::mobile::{self, Entries, Probe, State};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Enum)]
pub enum MobileInventoryState {
    Read,
    Missing,
    Unsupported,
    TimedOut,
    Exited,
    InvalidOutput,
    TooLarge,
    Unknown,
}

#[derive(SimpleObject)]
pub struct MobileTools {
    pub received_at: DateTime<Utc>,
    pub apple: AppleMobileTools,
    pub android: AndroidMobileTools,
}

#[derive(SimpleObject)]
pub struct AppleMobileTools {
    pub simulator_runtimes: SimulatorRuntimeInventory,
    pub device_types: SimulatorDeviceTypeInventory,
}

#[derive(SimpleObject)]
pub struct AndroidMobileTools {
    pub sdk_packages: AndroidSdkInventory,
    pub emulator: AndroidEmulatorInventory,
}

#[derive(SimpleObject)]
pub struct SimulatorRuntimeInventory {
    pub state: MobileInventoryState,
    pub entries: Option<Vec<SimulatorRuntime>>,
}

#[derive(SimpleObject)]
pub struct SimulatorRuntime {
    pub identifier: String,
    pub version: String,
    pub available: bool,
}

#[derive(SimpleObject)]
pub struct SimulatorDeviceTypeInventory {
    pub state: MobileInventoryState,
    pub entries: Option<Vec<SimulatorDeviceType>>,
}

#[derive(SimpleObject)]
pub struct SimulatorDeviceType {
    pub identifier: String,
}

#[derive(SimpleObject)]
pub struct AndroidSdkInventory {
    pub state: MobileInventoryState,
    pub entries: Option<Vec<AndroidSdkPackage>>,
}

#[derive(SimpleObject)]
pub struct AndroidSdkPackage {
    pub path: String,
    pub revision: String,
}

#[derive(SimpleObject)]
pub struct AndroidEmulatorInventory {
    pub state: MobileInventoryState,
    pub version: Option<String>,
}

fn state<T>(probe: &Probe<T>) -> MobileInventoryState {
    match probe {
        Probe::Read(_) => MobileInventoryState::Read,
        Probe::Unavailable(state) => match state {
            State::Missing => MobileInventoryState::Missing,
            State::Unsupported => MobileInventoryState::Unsupported,
            State::TimedOut => MobileInventoryState::TimedOut,
            State::Exited => MobileInventoryState::Exited,
            State::InvalidOutput => MobileInventoryState::InvalidOutput,
            State::TooLarge => MobileInventoryState::TooLarge,
            State::Unknown => MobileInventoryState::Unknown,
        },
    }
}

fn entries<T, U>(probe: &Probe<Entries<T>>, convert: impl Fn(&T) -> U) -> Option<Vec<U>> {
    match probe {
        Probe::Read(found) => Some(found.entries.iter().map(convert).collect()),
        Probe::Unavailable(_) => None,
    }
}

impl MobileTools {
    pub(super) fn of(found: &mobile::MobileTools, received_at: DateTime<Utc>) -> Self {
        let apple = &found.apple;
        let android = &found.android;
        Self {
            received_at,
            apple: AppleMobileTools {
                simulator_runtimes: SimulatorRuntimeInventory {
                    state: state(&apple.simulator_runtimes),
                    entries: entries(&apple.simulator_runtimes, |entry| SimulatorRuntime {
                        identifier: entry.identifier.clone(),
                        version: entry.version.clone(),
                        available: entry.available,
                    }),
                },
                device_types: SimulatorDeviceTypeInventory {
                    state: state(&apple.device_types),
                    entries: entries(&apple.device_types, |entry| SimulatorDeviceType {
                        identifier: entry.identifier.clone(),
                    }),
                },
            },
            android: AndroidMobileTools {
                sdk_packages: AndroidSdkInventory {
                    state: state(&android.sdk_packages),
                    entries: entries(&android.sdk_packages, |entry| AndroidSdkPackage {
                        path: entry.path.clone(),
                        revision: entry.revision.clone(),
                    }),
                },
                emulator: AndroidEmulatorInventory {
                    state: state(&android.emulator),
                    version: match &android.emulator {
                        Probe::Read(version) => Some(version.version.clone()),
                        Probe::Unavailable(_) => None,
                    },
                },
            },
        }
    }
}

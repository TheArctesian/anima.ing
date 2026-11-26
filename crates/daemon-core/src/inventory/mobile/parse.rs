use serde::Deserialize;

use super::{
    DeviceType, Entries, INPUT_LIMIT, Probe, SdkPackage, SimulatorRuntime, State, Version,
    validation,
};
use crate::inventory::is_version;

#[derive(Deserialize)]
struct Runtimes {
    runtimes: Vec<Runtime>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Runtime {
    identifier: String,
    version: String,
    is_available: bool,
}

#[derive(Deserialize)]
struct Types {
    devicetypes: Vec<DeviceType>,
}

#[must_use]
pub fn simulator_runtimes(output: &[u8]) -> Probe<Entries<SimulatorRuntime>> {
    let parsed = json::<Runtimes>(output).map(|found| {
        found
            .runtimes
            .into_iter()
            .map(|entry| SimulatorRuntime {
                identifier: entry.identifier,
                version: entry.version,
                available: entry.is_available,
            })
            .collect()
    });
    finish(parsed, 128, |entry| {
        (
            &entry.identifier,
            validation::identifier(&entry.identifier) && is_version(&entry.version),
        )
    })
}

#[must_use]
pub fn device_types(output: &[u8]) -> Probe<Entries<DeviceType>> {
    finish(
        json::<Types>(output).map(|found| found.devicetypes),
        256,
        |entry| (&entry.identifier, validation::identifier(&entry.identifier)),
    )
}

fn json<T: serde::de::DeserializeOwned>(output: &[u8]) -> Result<T, State> {
    if output.len() > INPUT_LIMIT {
        return Err(State::TooLarge);
    }
    serde_json::from_slice(output).map_err(|_| State::InvalidOutput)
}

pub(super) fn finish<T>(
    parsed: Result<Vec<T>, State>,
    limit: usize,
    key: impl Fn(&T) -> (&str, bool),
) -> Probe<Entries<T>> {
    let mut entries = match parsed {
        Ok(entries) => entries,
        Err(state) => return Probe::Unavailable(state),
    };
    if entries.len() > limit {
        return Probe::Unavailable(State::TooLarge);
    }
    entries.sort_by(|one, other| key(one).0.cmp(key(other).0));
    let found = Probe::Read(Entries { entries });
    if found.valid(limit, key) {
        found
    } else {
        Probe::Unavailable(State::InvalidOutput)
    }
}

#[must_use]
pub fn emulator(output: &[u8]) -> Probe<Version> {
    if output.len() > INPUT_LIMIT {
        return Probe::Unavailable(State::TooLarge);
    }
    let version = std::str::from_utf8(output)
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("Android emulator version "))
        })
        .and_then(|text| text.split_whitespace().next())
        .filter(|text| is_version(text));
    version.map_or(Probe::Unavailable(State::InvalidOutput), |version| {
        Probe::Read(Version {
            version: version.to_owned(),
        })
    })
}

#[derive(Deserialize)]
struct Repository {
    #[serde(rename = "localPackage")]
    package: Package,
}

#[derive(Deserialize)]
struct Package {
    #[serde(rename = "@path")]
    path: String,
    revision: Revision,
}

#[derive(Deserialize)]
struct Revision {
    major: u32,
    #[serde(default)]
    minor: u32,
    #[serde(default)]
    micro: u32,
    preview: Option<u32>,
}

/// Installed SDK package manifests; repository download catalogues have no `localPackage`.
#[must_use]
pub fn sdk_packages(manifests: &[Vec<u8>]) -> Probe<Entries<SdkPackage>> {
    if manifests.len() > 512 || manifests.iter().map(Vec::len).sum::<usize>() > INPUT_LIMIT {
        return Probe::Unavailable(State::TooLarge);
    }
    let parsed = manifests
        .iter()
        .map(|bytes| {
            let text = std::str::from_utf8(bytes).map_err(|_| State::InvalidOutput)?;
            let found: Repository =
                quick_xml::de::from_str(text).map_err(|_| State::InvalidOutput)?;
            let revision = found.package.revision;
            let mut version = format!("{}.{}.{}", revision.major, revision.minor, revision.micro);
            if let Some(preview) = revision.preview {
                version = format!("{version}-rc{preview}");
            }
            Ok(SdkPackage {
                path: found.package.path,
                revision: version,
            })
        })
        .collect();
    finish(parsed, 512, |entry| {
        (
            &entry.path,
            validation::package(&entry.path) && is_version(&entry.revision),
        )
    })
}

//! Bounded reads of administrator settings and Docker's allowed configuration.
use std::{
    fmt,
    fs::File,
    io::{self, Read as _},
    os::unix::fs::MetadataExt as _,
    path::Path,
};

use rustix::fs::{self, Mode, OFlags};
use serde::{
    Deserialize, Deserializer,
    de::{IgnoredAny, MapAccess, Visitor},
};

use super::{Error, paths};
use crate::{
    Settings,
    settings::{self, Guarded, LARGEST_SETTINGS},
};

pub(super) fn settings(path: &Path) -> Result<Settings, Error> {
    let located = paths::resolve(path, 0, paths::Kind::Settings)?;
    let file = open(&located.real)?;
    let metadata = file.metadata()?;
    settings::guard(&Guarded {
        owner: metadata.uid(),
        mode: metadata.mode(),
        regular: metadata.is_file(),
    })
    .map_err(|_| Error::UntrustedPath)?;
    let bytes = bounded(file, Error::Settings(settings::SettingsFault::TooLarge))?;
    let text = std::str::from_utf8(&bytes).map_err(|_| settings::SettingsFault::NotJson)?;
    Ok(settings::read(text)?)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    auths: Option<Registries>,
}

struct Registries(Vec<String>);

impl<'de> Deserialize<'de> for Registries {
    fn deserialize<D: Deserializer<'de>>(from: D) -> Result<Self, D::Error> {
        struct Names;
        impl<'de> Visitor<'de> for Names {
            type Value = Registries;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a map of registry names")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Registries, M::Error> {
                let mut names = Vec::new();
                while let Some(name) = map.next_key()? {
                    map.next_value::<IgnoredAny>()?;
                    names.push(name);
                }
                Ok(Registries(names))
            }
        }
        from.deserialize_map(Names)
    }
}

pub(super) fn docker(path: &Path, owner: u32) -> Result<(Option<Vec<String>>, Vec<String>), Error> {
    let path = path.join("config.json");
    let resolved = match paths::resolve(&path, owner, paths::Kind::ConfigurationFile) {
        Ok(resolved) => resolved,
        Err(Error::Io(io::ErrorKind::NotFound))
            if path
                .symlink_metadata()
                .is_err_and(|error| error.kind() == io::ErrorKind::NotFound) =>
        {
            return Ok((None, Vec::new()));
        }
        Err(error) => return Err(error),
    };
    let file = open(&resolved.real)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.uid() != owner || metadata.mode() & 0o022 != 0 {
        return Err(Error::UntrustedPath);
    }
    let bytes = bounded(file, Error::Configuration)?;
    let config: Configuration = serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
    let names = config
        .auths
        .map_or_else(Vec::new, |registries| registries.0);
    Ok((Some(vec!["auths".into()]), names))
}

fn open(path: &Path) -> Result<File, Error> {
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    Ok(File::from(fs::open(path, flags, Mode::empty())?))
}

fn bounded(file: File, too_large: Error) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    file.take(LARGEST_SETTINGS as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > LARGEST_SETTINGS {
        return Err(too_large);
    }
    Ok(bytes)
}

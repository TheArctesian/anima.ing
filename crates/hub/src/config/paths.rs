//! Host paths and remote-container paths have different filesystem rules.
use super::Error;
use std::{
    collections::BTreeMap,
    path::{Component, PathBuf},
};

/// An empty value (Compose passes `VAR=`) counts as unset.
pub(super) fn optional_path(env: &BTreeMap<String, String>, name: &str) -> Option<PathBuf> {
    env.get(name)
        .filter(|text| !text.is_empty())
        .map(PathBuf::from)
}

/// The root `/` is refused, as a web root would serve the whole file system, and so is any `..`
/// part.
pub(super) fn optional_root(
    env: &BTreeMap<String, String>,
    name: &'static str,
) -> Result<Option<PathBuf>, Error> {
    match env.get(name).map(String::as_str) {
        None | Some("") => Ok(None),
        Some(text) => {
            let path = absolute_path(name, text)?;
            if path.components().any(|part| part == Component::ParentDir) {
                return Err(Error::ParentDirectory { variable: name });
            }
            if path.parent().is_none() {
                return Err(Error::FilesystemRoot { variable: name });
            }
            Ok(Some(path))
        }
    }
}

pub(super) fn optional_socket(
    env: &BTreeMap<String, String>,
    name: &'static str,
) -> Result<Option<PathBuf>, Error> {
    match env.get(name).map(String::as_str) {
        None | Some("") => Ok(None),
        Some(text) => absolute_path(name, text).map(Some),
    }
}

fn absolute_path(variable: &'static str, text: &str) -> Result<PathBuf, Error> {
    let path = PathBuf::from(text);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(Error::NotAbsolute {
            variable,
            value: text.to_owned(),
        })
    }
}

/// The optional guacd container uses Unix paths even when this hub runs on Windows.
pub(super) fn remote_drive_path(variable: &'static str, text: &str) -> Result<PathBuf, Error> {
    if text.starts_with('/') {
        Ok(PathBuf::from(text))
    } else {
        Err(Error::NotAbsolute {
            variable,
            value: text.to_owned(),
        })
    }
}

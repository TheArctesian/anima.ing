//! Resolve the generation named by each managed systemd-boot entry.
use std::{fs, io::Read as _, path::Path};

use super::{Error, Format, open, require_generation};

/// Require the intended boot target and, when applicable, a compatible generation.
///
/// # Errors
/// Unreadable, mismatched or unsupported entry.
pub fn require_entry(
    path: &Path,
    expected: Option<&str>,
    format: Option<Format>,
) -> Result<(), Error> {
    let file = open(path)?;
    let mut text = String::new();
    if !file.metadata()?.is_file() || file.take(65_537).read_to_string(&mut text)? > 65_536 {
        return Err(invalid());
    }
    let system = target(&text)?;
    if expected.is_some_and(|expected| system != expected) {
        return Err(invalid());
    }
    if let Some(format) = format {
        require_generation(Path::new(system), format)?;
    }
    Ok(())
}

/// Require compatibility for the entries the Anima OS loader wildcard can select.
///
/// # Errors
/// A managed entry cannot be read or targets an incompatible generation.
pub fn require_menu(entries: &Path, format: Format) -> Result<(), Error> {
    for entry in fs::read_dir(entries)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(invalid)?;
        if name
            .get(..6)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("anima-"))
            && Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("conf"))
        {
            require_entry(&entry.path(), None, Some(format))?;
        }
    }
    Ok(())
}

fn target(body: &str) -> Result<&str, Error> {
    let mut parameters = body
        .lines()
        .filter_map(|line| {
            let (key, value) = line.trim_start().split_once(char::is_whitespace)?;
            key.eq_ignore_ascii_case("options").then_some(value)
        })
        .flat_map(str::split_whitespace)
        .filter_map(|parameter| parameter.strip_prefix("init="));
    let system = parameters
        .next()
        .and_then(|init| init.strip_suffix("/init"))
        .filter(|system| valid_system(system))
        .ok_or_else(invalid)?;
    if parameters.next().is_some() {
        return Err(invalid());
    }
    Ok(system)
}

/// Validate the installed Anima OS Nix store generation identifier.
#[must_use]
pub fn valid_system(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("/nix/store/") else {
        return false;
    };
    let Some((hash, name)) = rest.split_once("-nixos-system-anima-") else {
        return false;
    };
    hash.len() == 32
        && hash
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && !name.is_empty()
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b".+_-".contains(&c))
}

fn invalid() -> Error {
    Error::Refused(
        "A boot entry does not match the intended generation. Inspect the boot menu before retrying.",
    )
}

//! Check the actual boot choice against the reconciled Hosting authority.
#[cfg(test)]
mod tests;

use std::path::Path;

use anima_os_maintenance::cutover::{
    APPLICATION_ROOT, Format, require_entry, require_generation, require_menu, required_format,
};

use crate::{Error, Result, feed};

pub(super) fn required() -> Result<Option<Format>> {
    Ok(required_format(Path::new(APPLICATION_ROOT))?)
}

pub(super) fn generation(system: &str, format: Format) -> Result<()> {
    if !feed::valid_system(system) {
        return Err(Error::Refused("The retained system path is invalid."));
    }
    Ok(require_generation(Path::new(system), format)?)
}

pub(super) fn fallback(entries: &Path, previous: &str, format: Option<Format>) -> Result<()> {
    require_entry(&entries.join("anima-current.conf"), Some(previous), format)?;
    if let Some(format) = format {
        require_menu(entries, format)?;
    }
    Ok(())
}

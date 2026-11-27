use super::Error;
use std::{collections::BTreeMap, ffi::OsString};

/// Collect process variables into a map. A non-unicode `ANIMA_*` name or value is an error;
/// other non-unicode variables are skipped.
///
/// # Errors
/// Returns [`Error::NotUnicode`] naming the offending `ANIMA_*` variable.
pub fn collect_env(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<BTreeMap<String, String>, Error> {
    let mut env = BTreeMap::new();
    for (name, value) in vars {
        let ours = name.as_encoded_bytes().starts_with(b"ANIMA_");
        match (name.into_string(), value.into_string()) {
            (Ok(name), Ok(value)) => {
                env.insert(name, value);
            }
            (name, _) if ours => {
                let name = name.unwrap_or_else(|raw| raw.to_string_lossy().into_owned());
                return Err(Error::NotUnicode { name });
            }
            _ => {}
        }
    }
    Ok(env)
}

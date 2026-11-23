//! HMD-043: iOS answers "iPhone" for each phone, so the app makes a name once and keeps it; it is not a secret.

use std::{fs, path::Path};

use super::name;

const FILE: &str = "cloud-device";

/// A name that cannot be kept is new at each start, and the cloud then gives the device a new invite.
pub(super) fn device_name(
    folder: Option<&Path>,
    kind: &str,
    random: impl FnOnce() -> Option<[u8; 3]>,
) -> String {
    let file = folder.map(|folder| folder.join(FILE));
    if let Some(kept) = file.as_deref().and_then(kept) {
        return kept;
    }
    let made = match random() {
        Some([a, b, c]) => name::device_name(&format!("{kind}-{a:02x}{b:02x}{c:02x}")),
        None => name::device_name(kind),
    };
    if let Some(file) = file {
        let _ = fs::write(file, &made);
    }
    made
}

/// Only a name that the cloud takes.
fn kept(file: &Path) -> Option<String> {
    let text = fs::read_to_string(file).ok()?;
    let text = text.trim();
    (!text.is_empty() && name::device_name(text) == text).then(|| text.to_owned())
}

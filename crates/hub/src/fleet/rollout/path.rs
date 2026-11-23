//! The release that a machine gets next on its way to a version (ROL-041).

use super::model::{Release, Target};
use crate::fleet::builds::Version;

/// `aim` when its release can replace `installed`, else the highest held release below it that
/// can; `None` when the release of `aim` is not held or none can replace `installed`.
#[must_use]
pub fn next_hop(
    installed: Version,
    target: Target,
    aim: Version,
    held: &[Release],
) -> Option<Version> {
    let of_target = || held.iter().filter(|release| release.target == target);
    if installed >= aim || !of_target().any(|release| release.version == aim) {
        return None;
    }
    of_target()
        .filter(|release| release.version > installed && release.version <= aim)
        .filter(|release| release.replaces_from <= installed)
        .map(|release| release.version)
        .max()
}

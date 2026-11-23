//! The answer to `uname -s` is hostile device text: it is never decoded, logged or stored
//! (DEV-025).

use crate::fleet::devices::OperatingSystem;

pub(super) fn operating_system(uname: &[u8]) -> Option<OperatingSystem> {
    match uname.trim_ascii() {
        b"Linux" => Some(OperatingSystem::Linux),
        b"Darwin" => Some(OperatingSystem::Macos),
        _ => None,
    }
}

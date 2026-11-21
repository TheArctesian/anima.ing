//! Whether the daemon of this computer waits for the person to allow it in the macOS Keychain
//! (R-322). The daemon cannot tell the hub (without its token it cannot reach
//! it), so it puts the empty file [`MARK`] in its own folder while it waits and deletes it when
//! the Keychain answers; `anima-daemon install` puts it too when its read of the token waits.
//! The app only looks for the file: the client then shows that the daemon needs the person.

use std::path::Path;

use tauri::{Runtime, Webview};

use super::{DaemonError, from_main};

/// The name of the file, the same as `keychain_wait::MARK` of the daemon.
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "read only on macOS and by its tests")
)]
pub const MARK: &str = "keychain-waits";

/// Whether the daemon's folder `folder` holds the mark.
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "read only on macOS and by its tests")
)]
pub fn waits_in(folder: &Path) -> bool {
    folder.join(MARK).is_file()
}

/// Whether the daemon of this computer waits for the person in the Keychain. Only macOS has a
/// Keychain; elsewhere it never waits.
#[tauri::command]
pub async fn daemon_keychain_waits<R: Runtime>(webview: Webview<R>) -> Result<bool, DaemonError> {
    from_main(webview.label())?;
    #[cfg(target_os = "macos")]
    {
        let home = super::home_folder()?;
        Ok(waits_in(&crate::link::daemon_folder(&home, true, None)))
    }
    #[cfg(not(target_os = "macos"))]
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn met_013_the_daemon_waits_while_its_folder_holds_the_mark() {
        let folder = tempfile::tempdir().expect("a folder");
        assert!(!waits_in(folder.path()));
        std::fs::write(folder.path().join(MARK), b"").expect("the mark");
        assert!(waits_in(folder.path()));
    }
}

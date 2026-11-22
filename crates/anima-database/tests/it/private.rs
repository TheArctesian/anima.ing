//! A folder of a test that only this user can change.

use tempfile::TempDir;

/// A new temporary folder that the group and others cannot write to, whatever the umask
/// (0002 on Ubuntu): a backup refuses a folder that others can write to.
pub fn folder() -> TempDir {
    let folder = tempfile::tempdir().unwrap();
    #[cfg(windows)]
    anima_windows::files::protect(folder.path()).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(
        folder.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    folder
}

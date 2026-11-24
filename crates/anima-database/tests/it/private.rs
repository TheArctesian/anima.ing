use tempfile::TempDir;

/// Not writable by group or others whatever the umask: a backup refuses a folder that others can write to.
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

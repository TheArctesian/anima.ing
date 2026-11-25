use super::sdk;
use daemon_core::inventory::mobile::{Entries, Probe, SdkPackage, State};

#[test]
fn sdk_scan_reports_installed_packages_and_refuses_links() {
    let root = tempfile::tempdir().unwrap();
    // macOS temporary directory aliases are resolved only for the synthetic test root.
    let root = root.path().canonicalize().unwrap();
    let folder = root.join("platforms/android-35");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("package.xml"), b"<repository><localPackage path='platforms;android-35'><revision><major>2</major></revision></localPackage></repository>").unwrap();
    std::os::unix::fs::symlink("/not-an-sdk-file", folder.join("package-content-link")).unwrap();
    assert_eq!(
        sdk::read(&root),
        Probe::Read(Entries {
            entries: vec![SdkPackage {
                path: "platforms;android-35".into(),
                revision: "2.0.0".into(),
            }]
        })
    );
    std::fs::rename(folder.join("package.xml"), folder.join("source.xml")).unwrap();
    std::os::unix::fs::symlink(folder.join("source.xml"), folder.join("package.xml")).unwrap();
    assert_eq!(sdk::read(&root), Probe::Unavailable(State::Exited));
}

#[test]
fn sdk_scan_distinguishes_missing_empty_invalid_and_oversized() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    assert_eq!(
        sdk::read(&root.join("missing")),
        Probe::Unavailable(State::Missing)
    );
    assert_eq!(sdk::read(&root), Probe::Read(Entries { entries: vec![] }));
    let folder = root.join("platform-tools");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("package.xml"), b"broken").unwrap();
    assert_eq!(sdk::read(&root), Probe::Unavailable(State::InvalidOutput));
    std::fs::write(folder.join("package.xml"), vec![b' '; 1024 * 1024 + 1]).unwrap();
    assert_eq!(sdk::read(&root), Probe::Unavailable(State::TooLarge));
}

#[test]
fn sdk_discovery_stops_at_the_depth_bound() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("platforms/a/b/c/d/e/f/g/h")).unwrap();
    assert_eq!(sdk::read(&root), Probe::Unavailable(State::TooLarge));
}

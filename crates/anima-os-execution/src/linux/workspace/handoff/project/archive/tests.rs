use super::{Limits, open_project, write};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tar::EntryType;
use uuid::Uuid;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!("anima-project-{}", Uuid::now_v7()));
        fs::DirBuilder::new().create(&home).unwrap();
        fs::create_dir(home.join("workspace")).unwrap();
        Self(home)
    }

    fn capture(&self, limit: u64, output: &mut Vec<u8>) -> super::Result<(String, u64, usize)> {
        let limits = Limits {
            quota: 32 * 1024,
            bytes: limit,
            deadline: Instant::now() + Duration::from_secs(10),
        };
        write(&open_project(&self.0)?, output, limits)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn archive_preserves_project_edits_and_links_while_keeping_provider_home_out() {
    let home = Fixture::new();
    let project = home.0.join("workspace");
    fs::create_dir(project.join(".git")).unwrap();
    fs::create_dir(project.join("src")).unwrap();
    fs::create_dir(home.0.join(".codex")).unwrap();
    fs::write(home.0.join(".codex/auth.json"), b"fixture profile").unwrap();
    fs::write(project.join(".git/index"), b"current index").unwrap();
    fs::write(project.join(".env.local"), b"fixture project data").unwrap();
    let edited = project.join("src/run");
    fs::write(&edited, b"uncommitted code\n").unwrap();
    fs::set_permissions(&edited, fs::Permissions::from_mode(0o751)).unwrap();
    fs::hard_link(&edited, project.join("src/run-again")).unwrap();
    let link_target = format!("../{}.env.local", "./".repeat(60));
    symlink(&link_target, project.join("src/settings")).unwrap();
    let mut bytes = Vec::new();
    let receipt = home.capture(32768, &mut bytes).unwrap();
    let mut repeat = Vec::new();
    assert_eq!(home.capture(receipt.1, &mut repeat).unwrap(), receipt);
    assert_eq!(repeat, bytes);
    assert_eq!(receipt.1, bytes.len() as u64);
    assert_eq!(receipt.2, 7);
    assert!(home.capture(receipt.1 - 512, &mut Vec::new()).is_err());

    let mut data = BTreeMap::new();
    let mut archive = tar::Archive::new(bytes.as_slice());
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        let name = entry.path().unwrap().into_owned();
        assert!(!name.starts_with(".codex"));
        let kind = entry.header().entry_type();
        let mode = entry.header().mode().unwrap();
        let link = entry.link_name_bytes().map(std::borrow::Cow::into_owned);
        let mut content = Vec::new();
        entry.read_to_end(&mut content).unwrap();
        data.insert(name, (kind, mode, link, content));
    }
    assert_eq!(data[Path::new(".git/index")].3, b"current index");
    assert_eq!(data[Path::new(".env.local")].3, b"fixture project data");
    assert_eq!(data[Path::new("src/run")].1, 0o751);
    assert_eq!(data[Path::new("src/run")].3, b"uncommitted code\n");
    assert_eq!(data[Path::new("src/run-again")].0, EntryType::Link);
    assert_eq!(
        data[Path::new("src/run-again")].2.as_deref(),
        Some(b"src/run".as_slice())
    );
    assert_eq!(
        data[Path::new("src/settings")].2.as_deref(),
        Some(link_target.as_bytes())
    );
}

#[test]
fn archive_refuses_a_link_that_would_leave_the_project() {
    let home = Fixture::new();
    symlink("../.codex", home.0.join("workspace/profile")).unwrap();
    assert!(home.capture(32768, &mut Vec::new()).is_err());
    fs::remove_file(home.0.join("workspace/profile")).unwrap();
    fs::rename(home.0.join("workspace"), home.0.join("project")).unwrap();
    symlink("project", home.0.join("workspace")).unwrap();
    assert!(open_project(&home.0).is_err());
}

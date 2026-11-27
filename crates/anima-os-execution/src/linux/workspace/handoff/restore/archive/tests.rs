use super::{Receipt, unpack};
use crate::linux::workspace::handoff::project::archive::{Limits, open_project, write};
use agent_threads::managed::workspace::handoff::{Binding, Source};
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::PathBuf,
    time::{Duration, Instant},
};
use tar::{Builder, EntryType, Header};
use uuid::Uuid;

const QUOTA: u64 = 1 << 30;

struct Home(PathBuf);

impl Home {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("anima-restore-{}", Uuid::now_v7()));
        fs::create_dir(&dir).unwrap();
        fs::create_dir(dir.join("workspace")).unwrap();
        Self(dir)
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn receipt(bytes: &[u8], entries: usize) -> Receipt {
    Receipt {
        schema: 1,
        source: Source {
            workspace: Uuid::from_u128(1),
            principal: Uuid::from_u128(2),
            generation: 7,
            binding: Binding {
                transfer: Uuid::from_u128(3),
                destination: Uuid::from_u128(4),
            },
        },
        digest: format!(
            "sha256:{}",
            crate::linux::digests::hex(&Sha256::digest(bytes))
        ),
        bytes: bytes.len() as u64,
        entries,
    }
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(10)
}

fn checkpoint_fixture() -> (Vec<u8>, Receipt, String) {
    let source = Home::new();
    let project = source.0.join("workspace");
    fs::create_dir(project.join(".git")).unwrap();
    fs::write(project.join(".git/index"), b"fixture index").unwrap();
    fs::write(project.join("edited"), b"uncommitted change").unwrap();
    fs::set_permissions(project.join("edited"), fs::Permissions::from_mode(0o751)).unwrap();
    fs::hard_link(project.join("edited"), project.join("second")).unwrap();
    fs::write(project.join("x".repeat(120)), b"long filename").unwrap();
    let link = format!("{}edited", "./".repeat(60));
    symlink(&link, project.join("link")).unwrap();
    fs::create_dir(source.0.join(".codex")).unwrap();
    fs::write(source.0.join(".codex/auth.json"), b"source profile").unwrap();
    let mut bytes = Vec::new();
    let limits = Limits {
        quota: QUOTA,
        bytes: QUOTA,
        deadline: deadline(),
    };
    let (digest, _, entries) =
        write(&open_project(&source.0).unwrap(), &mut bytes, limits).unwrap();
    let expected = receipt(&bytes, entries);
    assert_eq!(expected.digest, digest);
    (bytes, expected, link)
}

#[test]
#[ignore = "restores Linux workspace UID/GID and requires a disposable root test process"]
fn producer_round_trip_restores_edits_git_and_links_with_fresh_home() {
    let (bytes, expected, link) = checkpoint_fixture();
    let home = Home::new();
    let restored = home.0.join("workspace");
    unpack(
        bytes.as_slice(),
        open_project(&home.0).unwrap(),
        (&expected, QUOTA),
        deadline(),
    )
    .unwrap();
    assert_eq!(
        fs::read(restored.join(".git/index")).unwrap(),
        b"fixture index"
    );
    assert_eq!(
        fs::read(restored.join("edited")).unwrap(),
        b"uncommitted change"
    );
    assert_eq!(
        fs::read(restored.join("x".repeat(120))).unwrap(),
        b"long filename"
    );
    assert_eq!(
        fs::read_link(restored.join("link"))
            .unwrap()
            .into_os_string(),
        link.as_str()
    );
    let edited = fs::metadata(restored.join("edited")).unwrap();
    assert_eq!(edited.mode() & 0o777, 0o751);
    assert_eq!(
        edited.ino(),
        fs::metadata(restored.join("second")).unwrap().ino()
    );
    assert_eq!((edited.uid(), edited.gid()), (1000, 1000));
    assert!(!home.0.join(".codex").exists());

    let fresh = Home::new();
    let mut changed = expected;
    changed.digest.replace_range(
        7..8,
        if changed.digest.as_bytes()[7] == b'a' {
            "b"
        } else {
            "a"
        },
    );
    assert!(
        unpack(
            bytes.as_slice(),
            open_project(&fresh.0).unwrap(),
            (&changed, QUOTA),
            deadline()
        )
        .is_err()
    );
}

#[test]
fn extraction_rejects_unsafe_links_and_oversized_extensions() {
    let home = Home::new();
    let mut base = Builder::new(Vec::new());
    let owned = |kind, size| {
        let mut header = Header::new_gnu();
        header.set_entry_type(kind);
        header.set_mode(0o700);
        header.set_uid(1000);
        header.set_gid(1000);
        header.set_size(size);
        header
    };
    base.append_data(&mut owned(EntryType::Directory, 0), ".", io::empty())
        .unwrap();
    let mut extension = Builder::new(base.get_ref().clone());
    let data = vec![b'x'; 4097];
    extension
        .append_data(
            &mut owned(EntryType::GNULongName, 4097),
            "@LongLink",
            data.as_slice(),
        )
        .unwrap();
    base.append_link(&mut owned(EntryType::Symlink, 0), "profile", "../.codex")
        .unwrap();
    for archive in [base.into_inner().unwrap(), extension.into_inner().unwrap()] {
        assert!(
            unpack(
                archive.as_slice(),
                open_project(&home.0).unwrap(),
                (&receipt(&archive, 1), QUOTA),
                deadline()
            )
            .is_err()
        );
    }
    assert_eq!(fs::read_dir(home.0.join("workspace")).unwrap().count(), 0);
}

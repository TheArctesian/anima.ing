use super::*;
use crate::clock::FixedClock;
use chrono::{DateTime, Utc};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};

fn offer() -> Offer {
    Offer {
        job: Uuid::now_v7(),
        attempt: 1,
        bytes: 3,
        archive_digest: "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            .into(),
        manifest_digest: format!("sha256:{}", "1".repeat(64)),
    }
}

async fn upload(store: &Store, device: Uuid, offer: Offer) -> Upload {
    match store.begin(device, offer).await.unwrap() {
        Intake::Upload(upload) => upload,
        Intake::Retained(_) => panic!("unexpected retained output"),
    }
}

async fn retain(verified: Verified) -> Receipt {
    verified
        .retain_before(
            Arc::new(FixedClock(DateTime::UNIX_EPOCH)),
            DateTime::<Utc>::MAX_UTC,
        )
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn private_receipts_survive_restart_bind_identity_and_reverify_retained_bytes() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("artifacts");
    let store = Store::open(&root, 6, 1).unwrap();
    let device = Uuid::now_v7();
    let offer = offer();
    let verified = upload(&store, device, offer.clone())
        .await
        .receive(b"abc".as_slice())
        .await
        .unwrap();
    assert!(
        !root.join(super::model::directory(&offer)).exists(),
        "verified staging is not retention"
    );
    assert!(matches!(
        store.begin(device, offer.clone()).await,
        Err(Error::Busy)
    ));
    let original = retain(verified).await;
    drop(store);
    let store = Store::open(&root, 6, 1).unwrap();
    let Intake::Retained(replayed) = store.begin(device, offer.clone()).await.unwrap() else {
        panic!("receipt missing after restart");
    };
    assert_eq!(replayed, original);
    assert!(matches!(
        store.begin(Uuid::now_v7(), offer.clone()).await,
        Err(Error::Conflict)
    ));
    let mut changed = offer.clone();
    changed.manifest_digest = format!("sha256:{}", "2".repeat(64));
    assert!(matches!(
        store.begin(device, changed).await,
        Err(Error::Conflict)
    ));
    std::fs::write(
        root.join(super::model::directory(&offer))
            .join("archive.oci"),
        b"abd",
    )
    .unwrap();
    assert!(matches!(
        store.begin(device, offer).await,
        Err(Error::Invalid)
    ));
}

#[tokio::test]
async fn invalid_streams_never_create_retained_outputs() {
    for bytes in [b"ab".as_slice(), b"abcd", b"abd"] {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("artifacts");
        let store = Store::open(&root, 6, 1).unwrap();
        let offer = offer();
        let result = upload(&store, Uuid::now_v7(), offer.clone())
            .await
            .receive(bytes)
            .await;
        assert!(matches!(result, Err(Error::Invalid)));
        assert!(!root.join(super::model::directory(&offer)).exists());
    }
}

#[tokio::test]
async fn capacity_and_exclusive_ownership_bound_retention() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("artifacts");
    let store = Store::open(&root, 3, 1).unwrap();
    assert!(matches!(Store::open(&root, 3, 1), Err(Error::Busy)));
    let device = Uuid::now_v7();
    let verified = upload(&store, device, offer())
        .await
        .receive(b"abc".as_slice())
        .await
        .unwrap();
    retain(verified).await;
    assert!(matches!(
        store.begin(device, offer()).await,
        Err(Error::Capacity)
    ));
    drop(store);
    let store = Store::open(&root, 6, u64::MAX).unwrap();
    assert!(matches!(
        store.begin(device, offer()).await,
        Err(Error::Capacity)
    ));
}

#[test]
fn recovery_discards_only_private_uncommitted_layouts_and_refuses_unsafe_roots() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("artifacts");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let pending = root.join(".incoming-0123456789abcdef");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&pending)
        .unwrap();
    let unmanaged = root.join("unmanaged");
    std::fs::write(&unmanaged, b"keep").unwrap();
    assert!(Store::open(&root, 6, 1).is_err());
    assert!(
        pending.exists(),
        "validate the entire layout before deleting staging"
    );
    assert_eq!(std::fs::read(&unmanaged).unwrap(), b"keep");
    std::fs::remove_file(unmanaged).unwrap();
    let store = Store::open(&root, 6, 1).unwrap();
    assert!(!pending.exists());
    drop(store);
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&pending)
        .unwrap();
    let outside = parent.path().join("keep");
    std::fs::write(&outside, b"keep").unwrap();
    symlink(&outside, pending.join("archive.oci")).unwrap();
    assert!(matches!(Store::open(&root, 6, 1), Err(Error::Private)));
    assert_eq!(std::fs::read(outside).unwrap(), b"keep");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(Store::open(&root, 6, 1), Err(Error::Private)));
}

//! The store of originals: private files, a handle that is checked, a limit of age and a
//! limit of size (SHA-010 to SHA-014).

use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

use super::*;

fn original(answer: Value) -> Original {
    Original {
        tool: "t".to_owned(),
        answer,
        lists: vec!["events".to_owned()],
    }
}

fn store() -> (tempfile::TempDir, Store) {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let store = Store::new(folder.path().join("shortening"));
    (folder, store)
}

#[test]
fn sha_011_an_original_comes_back_whole_by_its_handle() {
    let (_folder, store) = store();
    let answer = json!({ "events": [{ "message": "disk is full" }], "nextCursor": null });
    let now = SystemTime::now();
    let handle = store.put(&original(answer.clone()), now).expect("stored");
    assert!(is_handle(&handle), "{handle}");
    assert_eq!(handle, handle_of(&answer.to_string()));
    let read = store.get(&handle, now).expect("read");
    assert_eq!(read, original(answer));
}

#[cfg(unix)]
#[test]
fn sha_010_the_folder_and_the_file_are_for_the_user_only() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_folder, store) = store();
    let handle = store
        .put(&original(json!({ "a": 1 })), SystemTime::now())
        .expect("stored");
    let mode = |path: &Path| fs::metadata(path).expect("there").permissions().mode() & 0o777;
    assert_eq!(mode(store.dir()), 0o700);
    assert_eq!(mode(&store.dir().join(format!("{handle}.json"))), 0o600);
}

#[test]
fn sha_011_a_text_that_is_not_a_handle_is_refused_before_the_disk() {
    let (folder, store) = store();
    fs::write(folder.path().join("secret.json"), "{}").expect("written");
    let now = SystemTime::now();
    for text in [
        "../secret",
        "../../etc/passwd",
        "",
        "ABCDEF0123456789abcdef01",
        &"a".repeat(25),
        "0123456789abcdef0123456/",
    ] {
        assert!(
            matches!(store.get(text, now), Err(OriginalError::NotAHandle)),
            "{text}"
        );
    }
    assert!(matches!(
        store.get(&"0".repeat(24), now),
        Err(OriginalError::Gone)
    ));
}

#[test]
fn sha_012_an_original_older_than_a_day_is_gone_and_removed() {
    let (_folder, store) = store();
    let now = SystemTime::now();
    let handle = store
        .put(&original(json!({ "a": 1 })), now)
        .expect("stored");
    let later = now + OLDEST + Duration::from_mins(1);
    assert!(matches!(
        store.get(&handle, later),
        Err(OriginalError::Gone)
    ));
    assert_eq!(store.list().expect("listed"), []);
    let again = store
        .put(&original(json!({ "a": 1 })), now)
        .expect("stored");
    store
        .put(&original(json!({ "b": 2 })), later)
        .expect("stored");
    let handles: Vec<String> = store
        .list()
        .expect("listed")
        .into_iter()
        .map(|kept| kept.handle)
        .collect();
    assert!(
        !handles.contains(&again),
        "the old original is removed when a new one comes"
    );
    assert_eq!(handles.len(), 1);
}

#[test]
fn sha_012_an_original_above_the_limit_of_one_file_is_not_stored() {
    let (_folder, store) = store();
    let large = json!({ "text": "x".repeat(private_file::LIMIT) });
    assert!(matches!(
        store.put(&original(large), SystemTime::now()),
        Err(OriginalError::TooLarge)
    ));
    assert_eq!(store.list().expect("listed"), []);
}

#[test]
fn sha_015_clear_removes_the_originals_and_no_other_file() {
    let (_folder, store) = store();
    let now = SystemTime::now();
    store
        .put(&original(json!({ "a": 1 })), now)
        .expect("stored");
    store
        .put(&original(json!({ "a": 2 })), now)
        .expect("stored");
    private_file::write(store.dir(), "state", b"on").expect("written");
    assert_eq!(store.clear().expect("cleared"), 2);
    assert!(store.dir().join("state").exists());
}

#[cfg(unix)]
#[test]
fn sha_010_a_link_or_a_folder_with_the_name_of_an_original_is_not_one() {
    let (folder, store) = store();
    let now = SystemTime::now();
    store
        .put(&original(json!({ "a": 1 })), now)
        .expect("stored");
    let secret = folder.path().join("secret");
    fs::write(&secret, r#"{"tool":"t","answer":{"token":"x"}}"#).expect("written");
    let link = "a".repeat(24);
    std::os::unix::fs::symlink(&secret, store.dir().join(format!("{link}.json"))).expect("linked");
    fs::create_dir(store.dir().join(format!("{}.json", "b".repeat(24)))).expect("made");
    assert!(matches!(store.get(&link, now), Err(OriginalError::Gone)));
    assert_eq!(store.list().expect("listed").len(), 1);
    store
        .put(&original(json!({ "a": 2 })), now)
        .expect("a foreign entry does not stop the store");
    assert_eq!(store.clear().expect("cleared"), 2);
    assert!(secret.exists());
}

#[test]
fn sha_014_a_file_that_is_not_a_stored_original_is_gone_and_no_panic() {
    let (_folder, store) = store();
    let handle = "c".repeat(24);
    for body in ["[1]", "\"x\"", "{}", "not json"] {
        private_file::write(store.dir(), &format!("{handle}.json"), body.as_bytes())
            .expect("written");
        assert!(
            matches!(
                store.get(&handle, SystemTime::now()),
                Err(OriginalError::Gone)
            ),
            "{body}"
        );
    }
}

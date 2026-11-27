use super::*;
use crate::agent_threads::folders;
use std::fs;

/// Real backend events over made directories: creation, append, replacement and cancellation.
#[test]
fn session_changes_survive_missing_and_replaced_folders() {
    let home = tempfile::Builder::new()
        .prefix("anima-watch-")
        .tempdir()
        .expect("temporary home");
    let home = home.path().canonicalize().expect("canonical home");
    let running = Arc::new(AtomicBool::new(true));
    let worker = Arc::clone(&running);
    let (send, receive) = mpsc::channel();
    let root = home.clone();
    let task = std::thread::spawn(move || {
        watch(&root, &worker, |healthy| send.send(healthy).is_ok()).expect("watch stays available");
        true
    });
    assert_eq!(
        receive.recv_timeout(Duration::from_secs(5)).expect("ready"),
        Change::Ready
    );
    let session = home.join(".codex/sessions/2026/11/23");
    fs::create_dir_all(&session).expect("made session folders");
    let file = session.join("rollout-synthetic.jsonl");
    fs::write(&file, "{}\n").expect("made session");
    assert_eq!(
        receive
            .recv_timeout(Duration::from_secs(5))
            .expect("new session"),
        Change::Changed
    );
    // Drain the directory-creation burst before testing an existing file's change.
    while receive.recv_timeout(Duration::from_millis(600)).is_ok() {}
    fs::write(&file, "{\"synthetic\":true}\n").expect("changed session");
    assert_eq!(
        receive
            .recv_timeout(Duration::from_secs(5))
            .expect("changed file"),
        Change::Changed
    );
    while receive.recv_timeout(Duration::from_millis(600)).is_ok() {}
    fs::remove_dir_all(home.join(".codex")).expect("remove synthetic folders");
    fs::create_dir_all(&session).expect("replace synthetic folders");
    fs::write(&file, "{}\n").expect("replacement session");
    assert_eq!(
        receive
            .recv_timeout(Duration::from_secs(5))
            .expect("replaced session"),
        Change::Changed
    );
    while receive.recv_timeout(Duration::from_millis(600)).is_ok() {}
    fs::write(&file, "{\"again\":true}\n").expect("change replacement session");
    assert_eq!(
        receive
            .recv_timeout(Duration::from_secs(5))
            .expect("replacement stays watched"),
        Change::Changed
    );
    running.store(false, Ordering::Release);
    assert!(task.join().expect("watch worker"));
}

#[test]
fn symlinked_session_roots_are_not_registered() {
    let home = tempfile::tempdir().expect("temporary home");
    let outside = tempfile::tempdir().expect("outside folder");
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), home.path().join(".claude"))
        .expect("synthetic link");
    let folders = folders::discover(home.path()).expect("bounded discovery");
    assert_eq!(
        folders.keys().collect::<Vec<_>>(),
        vec![&home.path().to_path_buf()]
    );
}

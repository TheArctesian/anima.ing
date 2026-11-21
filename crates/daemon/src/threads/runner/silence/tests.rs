//! The tests of the version question and of the wait for the first answer (TH97).

use super::*;

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("a script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
    path
}

#[tokio::test]
async fn th97_a_copy_that_does_not_answer_is_named_with_a_copy_that_does() {
    let dir = tempfile::tempdir().expect("a folder");
    let silent = script(dir.path(), "codex", "exec sleep 600");
    let other = dir.path().join("app");
    std::fs::create_dir(&other).expect("a folder");
    let answers = script(&other, "codex", "echo 'codex-cli 0.169.0'");
    let env = vec![("PATH".to_owned(), "/usr/bin:/bin".to_owned())];
    let copies = vec![silent.clone(), answers.clone()];
    let mut silence = Silence::default();
    assert_eq!(silence.kept(&silent), None, "not asked yet");
    let started = Instant::now();
    let asked = ask(silent.clone(), copies.clone(), env.clone()).await;
    assert!(started.elapsed() < PROBE_WAIT + Duration::from_secs(10));
    silence.keep(&asked);
    assert_eq!(silence.kept(&silent), Some(false), "kept, not asked again");
    assert_eq!(silence.kept(&answers), Some(true));
    let words = not_answered(
        Program::Codex,
        &silent,
        &silence.answered_others(&silent, &copies),
    );
    assert!(words.contains(&format!("The Codex at {}", silent.display())));
    assert!(words.contains(&format!("{} (codex-cli 0.169.0)", answers.display())));
    let fine = ask(answers.clone(), copies, env).await;
    assert_eq!(fine.len(), 1, "the others are asked only after a silence");
}

#[test]
fn th97_a_copy_changed_since_it_answered_or_not_utf8_is_not_offered() {
    use std::os::unix::ffi::OsStrExt as _;
    let dir = tempfile::tempdir().expect("a folder");
    let silent = script(dir.path(), "codex", "exec sleep 600");
    let replaced = script(dir.path(), "codex-app", "echo 'codex-cli 0.169.0'");
    // A Mac takes no file name that is not UTF-8; the path alone is enough here.
    let odd = dir.path().join(std::ffi::OsStr::from_bytes(b"codex-\xff"));
    let kept = script(dir.path(), "codex-kept", "echo 'codex-cli 0.169.0'");
    let mut silence = Silence::default();
    let answered = |path: &PathBuf| (path.clone(), Some("codex-cli 0.169.0".to_owned()));
    silence.keep(&vec![answered(&replaced), answered(&odd), answered(&kept)]);
    // An update writes a new file in place of the old one.
    let fresh = dir.path().join("fresh");
    std::fs::copy(&replaced, &fresh).expect("copied");
    std::fs::rename(&fresh, &replaced).expect("put in place");
    let copies = vec![silent.clone(), replaced, odd, kept.clone()];
    let offered: Vec<PathBuf> = silence
        .answered_others(&silent, &copies)
        .into_iter()
        .map(|(copy, _)| copy)
        .collect();
    assert_eq!(offered, [kept]);
}

#[test]
fn th97_a_thread_still_starting_after_the_limit_is_ended_with_words() {
    let mut silence = Silence::default();
    let (late, early, working) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let path = PathBuf::from("/opt/homebrew/bin/codex");
    for id in [late, early, working] {
        silence.started(id, Program::Codex, path.clone());
    }
    let now = chrono::Utc::now();
    let old = now - chrono::Duration::minutes(11);
    let threads = [
        (late, RunState::Starting, old),
        (
            early,
            RunState::Starting,
            now - chrono::Duration::minutes(1),
        ),
        (working, RunState::Working, old),
    ];
    assert_eq!(silence.overdue(&threads, now), [late]);
    assert_eq!(silence.overdue(&threads, now), Vec::<Uuid>::new(), "once");
    let words = silence.ended(late).expect("words");
    assert!(words.starts_with("The Codex at /opt/homebrew/bin/codex said nothing in 10 minutes"));
    assert_eq!(silence.ended(working), None);
}

#[test]
fn th97_the_chosen_copy_is_named_only_when_there_are_more() {
    let first = PathBuf::from("/opt/homebrew/bin/codex");
    let app = PathBuf::from("/Applications/ChatGPT.app/codex");
    assert_eq!(
        chosen(Program::Codex, &first, false, std::slice::from_ref(&first)),
        None
    );
    let words = chosen(Program::Codex, &first, false, &[first.clone(), app]).expect("words");
    assert!(words.contains("first on the PATH of your login"));
    assert!(words.ends_with("/Applications/ChatGPT.app/codex."));
}

//! Linux gates do not compile `desktop/windows.rs`, so this test keeps its command list in step with `desktop.rs` (WIN1).

use std::collections::BTreeSet;

const NOT_ON_WINDOWS: [(&str, &str); 6] = [
    (
        "daemon::at_start::daemon_update_notice",
        "tells the page what the update of the daemon at the start did; module daemon::at_start is Unix only (HMD-055)",
    ),
    (
        "daemon::keychain::daemon_keychain_waits",
        "reads the mark of a daemon that waits in the macOS Keychain; module daemon::keychain is Unix only (R-322)",
    ),
    (
        "system_settings::sharing_settings_open",
        "opens the sharing settings of macOS or Linux; module system_settings is Unix only",
    ),
    (
        "agent_runner::agent_runner_call",
        "calls the thread runner of this computer over its Unix socket; module agent_runner is Unix only (THR-011)",
    ),
    (
        "agent_runner::agent_runner_watch",
        "watches the thread runner of this computer over its Unix socket; module agent_runner is Unix only (THR-011)",
    ),
    (
        "agent_runner::agent_thread_notify",
        "tells the person that a thread of the runner needs them; module agent_runner is Unix only (THR-011)",
    ),
];

const ONLY_ON_WINDOWS: [(&str, &str); 0] = [];

fn commands(source: &str) -> BTreeSet<&str> {
    let start = source.find("generate_handler![").expect("a command list");
    let list = &source[start..];
    let end = list.find(']').expect("the end of the command list");
    list["generate_handler![".len()..end]
        .split(',')
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .collect()
}

#[test]
fn win1_windows_registers_each_desktop_command_but_the_named_exceptions() {
    let desktop = commands(include_str!("../desktop.rs"));
    let windows = commands(include_str!("windows.rs"));
    let missing: Vec<_> = desktop
        .difference(&windows)
        .filter(|path| !NOT_ON_WINDOWS.iter().any(|(name, _)| name == *path))
        .collect();
    let extra: Vec<_> = windows
        .difference(&desktop)
        .filter(|path| !ONLY_ON_WINDOWS.iter().any(|(name, _)| name == *path))
        .collect();
    assert!(
        missing.is_empty(),
        "register these in desktop/windows.rs, or name them in NOT_ON_WINDOWS with the reason: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "register these in desktop.rs, or name them in ONLY_ON_WINDOWS with the reason: {extra:?}"
    );
}

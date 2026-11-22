//! What each run of `git` gets: its environment and its fixed configuration.

use std::ffi::OsString;

use crate::threads::git::run::{Asked, Git, fixed_configuration, fixed_environment};

#[test]
fn only_the_kept_names_of_the_environment_reach_git() {
    let env = [
        ("HOME", "/home/ada"),
        ("PATH", "/usr/bin"),
        ("GIT_DIR", "/elsewhere/.git"),
        ("GIT_INDEX_FILE", "/elsewhere/index"),
        ("ANIMA_DAEMON_TOKEN", "secret"),
        ("GH_TOKEN", "secret"),
    ]
    .map(|(name, value)| (OsString::from(name), OsString::from(value)));
    let Ok(git) = Git::find(env) else {
        return;
    };
    assert_eq!(git.env_names(), ["HOME", "PATH"]);
}

#[test]
fn the_runner_has_no_hooks_and_the_person_has_theirs() {
    assert!(fixed_configuration(Asked::Runner, true).contains(&"core.hooksPath=/dev/null"));
    // A run for the person has the hooks of the repository only when the thread allows them.
    assert!(fixed_configuration(Asked::Person, false).contains(&"core.hooksPath=/dev/null"));
    // No transport for the runner, and no fetch of a missing object for anyone: a remote
    // of a partial clone cannot name a program that a diff then starts.
    assert!(fixed_configuration(Asked::Runner, true).contains(&"protocol.allow=never"));
    assert!(!fixed_configuration(Asked::Person, true).contains(&"protocol.allow=never"));
    assert!(fixed_environment().contains(&("GIT_NO_LAZY_FETCH", "1")));
    assert!(!fixed_configuration(Asked::Person, true).contains(&"core.hooksPath=/dev/null"));
}

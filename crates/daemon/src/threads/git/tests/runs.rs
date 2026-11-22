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
    assert!(fixed_configuration(Asked::Runner).contains(&"core.hooksPath=/dev/null"));
    // No transport for the runner, and no fetch of a missing object for anyone: a remote
    // of a partial clone cannot name a program that a diff then starts.
    assert!(fixed_configuration(Asked::Runner).contains(&"protocol.allow=never"));
    assert!(!fixed_configuration(Asked::Person).contains(&"protocol.allow=never"));
    assert!(fixed_environment().contains(&("GIT_NO_LAZY_FETCH", "1")));
    // The hooks that the person set for all their repositories run for their own push.
    assert!(!fixed_configuration(Asked::Person).contains(&"core.hooksPath=/dev/null"));
    for asked in [Asked::Runner, Asked::Person] {
        assert!(fixed_configuration(asked).contains(&"protocol.ext.allow=never"));
    }
}

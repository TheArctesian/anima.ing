//! What each run of `git` gets whatever the repository says: the configuration on its command
//! line and its fixed environment (THR-050).
//!
//! Pure: lists of fixed text.

use super::Asked;

/// The configuration each run gets on its command line, where it wins over each file: no
/// monitor program, no external diff, no `ext::` transport, no pager, no optional lock, quoted
/// paths as bytes. A run of the runner itself has no hooks and no transport.
pub fn fixed_configuration(asked: Asked) -> Vec<&'static str> {
    let mut args = vec![
        "--no-pager",
        "--no-optional-locks",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.quotePath=false",
        "-c",
        "core.sshCommand=ssh -o BatchMode=yes",
        "-c",
        "protocol.ext.allow=never",
        "-c",
        "diff.external=",
        "-c",
        "advice.detachedHead=false",
        "-c",
        "gc.auto=0",
    ];
    if asked == Asked::Runner {
        // No hook, no transport (so no program of a remote, also not for a missing object of a
        // partial clone), and nothing read from a repository inside the worktree.
        args.extend([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "protocol.allow=never",
            "-c",
            "status.submoduleSummary=false",
            "-c",
            "diff.ignoreSubmodules=all",
            "-c",
            "submodule.recurse=false",
        ]);
    }
    args
}

/// The environment each run gets after the kept names: no prompt, fixed words, a fixed `ssh`,
/// and no fetch of an object that a partial clone lacks.
pub fn fixed_environment() -> [(&'static str, &'static str); 5] {
    [
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_SSH_COMMAND", "ssh -o BatchMode=yes"),
        ("GIT_PAGER", "cat"),
        ("LC_ALL", "C"),
        ("GIT_NO_LAZY_FETCH", "1"),
    ]
}

//! The web view cannot name a program or an argument, only a step (BLD-027, BLD-040 to BLD-043,
//! BLD-047).

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use super::job::Commit;

mod bundling;
pub use bundling::{app_archive, bundle, bundle_checks, notarize, staple};

/// BLD-040. The repository's own name, not one GitHub redirects from: a redirect ends when a
/// repository takes the old name.
pub const REPOSITORY: &str = "https://github.com/TheArctesian/anima.ing.git";
const MAIN: &str = "refs/remotes/origin/main";

/// The repository and the one transport git may use for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    url: String,
    scheme: &'static str,
}

impl Repository {
    pub fn anima() -> Self {
        Self {
            url: REPOSITORY.to_owned(),
            scheme: "https",
        }
    }

    #[cfg(all(test, unix))]
    pub fn local(path: &Path) -> Self {
        Self {
            url: format!("file://{}", path.display()),
            scheme: "file",
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    fn transport(&self) -> [String; 2] {
        [
            "-c".to_owned(),
            format!("protocol.{}.allow=always", self.scheme),
        ]
    }
}

/// A clone whose config holds any other key (`url.*.insteadOf`, an `include`, a filter) was changed
/// by a build and is cloned anew (D-15).
pub const CLONE_CONFIG_KEYS: &[&str] = &[
    "core.repositoryformatversion",
    "core.filemode",
    "core.bare",
    "core.logallrefupdates",
    "core.ignorecase",
    "core.precomposeunicode",
    "core.symlinks",
    "remote.origin.url",
    "remote.origin.fetch",
    "branch.main.remote",
    "branch.main.merge",
];

/// Found in the step's program folders, never in the clone; `Sh` is always `/bin/sh` (D-2 of the
/// S12 packaging tasks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Program {
    Git,
    Bun,
    Tar,
    Codesign,
    Lipo,
    Xcrun,
    Cargo,
    Go,
    Sh,
}

impl Program {
    pub fn name(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Bun => "bun",
            Self::Tar => "tar",
            Self::Codesign => "codesign",
            Self::Lipo => "lipo",
            Self::Xcrun => "xcrun",
            Self::Cargo => "cargo",
            Self::Go => "go",
            Self::Sh => "sh",
        }
    }

    pub fn fixed_path(self) -> Option<&'static Path> {
        (self == Self::Sh).then(|| Path::new("/bin/sh"))
    }
}

/// Both streams always go to the step's log; only this one is read for a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    Output,
    /// `codesign -d` prints what it displays on standard error.
    Errors,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub program: Program,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    /// Variables the step adds to the scrubbed environment (BLD-044).
    pub env: Vec<(&'static str, String)>,
    pub kept: Kept,
}

pub fn command(program: Program, cwd: &Path, args: &[&str]) -> Command {
    Command {
        program,
        args: args.iter().map(OsString::from).collect(),
        cwd: cwd.to_owned(),
        env: Vec::new(),
        kept: Kept::Output,
    }
}

/// A build writes to the clone, so its `.git/config` cannot turn hooks, credential helpers or
/// transports
/// back on (BLD-044). No automatic `gc`, so git never writes the commit graph that makes a clone
/// untrusted (D-21).
pub(super) fn git(cwd: &Path, args: &[&str]) -> Command {
    let mut all = vec![
        "-c",
        "gc.auto=0",
        "-c",
        "maintenance.auto=false",
        "-c",
        if cfg!(windows) {
            "core.hooksPath=NUL"
        } else {
            "core.hooksPath=/dev/null"
        },
        "-c",
        "core.autocrlf=false",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "credential.helper=",
        "-c",
        "protocol.allow=never",
        "-c",
        "submodule.recurse=false",
        "-c",
        "advice.detachedHead=false",
    ];
    all.extend_from_slice(args);
    command(Program::Git, cwd, &all)
}

fn git_over(repository: &Repository, cwd: &Path, args: &[&str]) -> Command {
    let mut step = git(cwd, &[]);
    step.args.extend(repository.transport().map(OsString::from));
    step.args.extend(args.iter().map(OsString::from));
    step
}

pub fn clone(repository: &Repository, clone: &Path) -> Command {
    let parent = clone.parent().unwrap_or(clone);
    let mut step = git_over(
        repository,
        parent,
        &["clone", "--no-checkout", "--", repository.url()],
    );
    step.args.push(clone.as_os_str().to_owned());
    step
}

/// By URL, never through a remote the clone's config could redirect (BLD-040, D-15).
pub fn fetch(repository: &Repository, clone: &Path) -> Command {
    git_over(
        repository,
        clone,
        &[
            "fetch",
            "--no-tags",
            "--",
            repository.url(),
            &format!("+refs/heads/main:{MAIN}"),
        ],
    )
}

pub fn config_keys(clone: &Path) -> Command {
    git(clone, &["config", "--local", "--name-only", "--list"])
}

/// An object a build forged cannot make another commit look like part of `main` (D-21).
pub fn fsck(clone: &Path) -> Command {
    git(clone, &["fsck", "--no-dangling", "--no-progress"])
}

/// Exits 0 only when `commit` is an ancestor of `main` (BLD-041).
pub fn ancestry(clone: &Path, commit: &Commit) -> Command {
    git(
        clone,
        &["merge-base", "--is-ancestor", commit.as_str(), MAIN],
    )
}

/// Keeps cargo's `target/` cache (BLD-040).
pub fn checkout(clone: &Path, commit: &Commit) -> Vec<Command> {
    vec![
        git(clone, &["checkout", "--force", "--detach", commit.as_str()]),
        git(clone, &["clean", "-ffdx", "--exclude=/target/"]),
    ]
}

/// An entry a build marked `--skip-worktree` or `--assume-unchanged` would hide a changed file from
/// `status`.
pub fn read_tree(clone: &Path, commit: &Commit) -> Command {
    git(clone, &["read-tree", commit.as_str()])
}

pub fn status(clone: &Path) -> Command {
    git(clone, &["status", "--porcelain", "--untracked-files=all"])
}

pub fn head(clone: &Path) -> Command {
    git(clone, &["rev-parse", "HEAD"])
}

/// `bun run gates` with `BASE_REF` set to the job commit (BLD-042). `DATABASE_URL` is never in
/// a step's environment, so the gates start their own PostgreSQL.
pub fn gates(clone: &Path, commit: &Commit) -> Command {
    let mut step = command(Program::Bun, clone, &["run", "gates"]);
    step.env.push(("BASE_REF", commit.as_str().to_owned()));
    step
}

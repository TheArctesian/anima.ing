//! S12 2.2, D-2. Go runs with a fixed environment, and modules come only from the public proxy,
//! each checked against the checksum database.

use std::path::{Path, PathBuf};

use super::{staged_as, triples};
use crate::builder::{
    job::Target,
    plan::{Command, Program, command},
};

pub const PROGRAM: &str = "anima-link";
const RECIPE: &str = "link/build.sh";
const OUTPUT: &str = "link/bin";
const MODULE: &str = "link";
/// Each is checked against the commit before the recipe runs or reads it.
pub const COMMITTED: [&str; 3] = [RECIPE, "link/go.mod", "link/go.sum"];
/// Files git ignores that would change what Go builds; none may be in the clone.
pub const ABSENT: [&str; 2] = ["link/go.work", "link/vendor"];
const GO_ENVIRONMENT: [(&str, &str); 11] = [
    ("GOENV", "off"),
    ("GOTOOLCHAIN", "local"),
    ("GOWORK", "off"),
    ("GOFLAGS", "-mod=readonly"),
    ("GOPROXY", "https://proxy.golang.org"),
    ("GOSUMDB", "sum.golang.org"),
    ("GONOSUMDB", ""),
    ("GONOSUMCHECK", ""),
    ("GONOPROXY", ""),
    ("GOINSECURE", ""),
    ("GOPRIVATE", ""),
];

fn with_go_environment(mut step: Command) -> Command {
    step.env.extend(
        GO_ENVIRONMENT
            .iter()
            .map(|(name, value)| (*name, (*value).to_owned())),
    );
    step
}

pub fn verify(clone: &Path) -> Command {
    with_go_environment(command(
        Program::Go,
        &clone.join(MODULE),
        &["mod", "verify"],
    ))
}

fn go_platform(triple: &str) -> String {
    let os = if triple.ends_with("-apple-darwin") {
        "darwin"
    } else if triple.ends_with("-windows-msvc") {
        "windows"
    } else {
        "linux"
    };
    let arch = if triple.starts_with("aarch64-") {
        "arm64"
    } else {
        "amd64"
    };
    format!("{os}-{arch}")
}

pub fn build(clone: &Path) -> Command {
    with_go_environment(command(Program::Sh, clone, &[RECIPE]))
}

pub fn copies(clone: &Path, target: Target) -> Vec<(PathBuf, PathBuf)> {
    triples(target)
        .iter()
        .map(|triple| {
            (
                clone.join(OUTPUT).join(format!(
                    "{PROGRAM}-{}{}",
                    go_platform(triple),
                    if triple.ends_with("-windows-msvc") {
                        ".exe"
                    } else {
                        ""
                    }
                )),
                staged_as(clone, PROGRAM, triple),
            )
        })
        .collect()
}

pub const WINDOWS_COMMITTED: [&str; 3] = ["link/build-windows.ts", "link/go.mod", "link/go.sum"];

pub fn build_for(clone: &Path, target: Target) -> Command {
    if target == Target::WindowsX86_64 {
        with_go_environment(command(Program::Bun, clone, &["link/build-windows.ts"]))
    } else {
        build(clone)
    }
}

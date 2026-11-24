//! The PostgreSQL tree a release carries (S3 T11, S12 2.2): the recipe runs by `/bin/sh` (D-2)
//! in a work folder made anew for each release (D-1). Pure.

use std::path::{Path, PathBuf};

use crate::builder::{
    error::BuilderError,
    job::Target,
    plan::{Command, Program, command},
};

const RECIPE: &str = "apps/desktop/src-tauri/database-tree/build.sh";
/// `bundle.resources` of `tauri.conf.json` names this folder.
const OUT: &str = "apps/desktop/src-tauri/database";
/// Beside the clone, so `git clean` of the clone never reaches it.
const WORK: &str = "database-tree";
/// Each is checked against the commit before the recipe runs.
pub const COMMITTED: [&str; 2] = [
    RECIPE,
    "apps/desktop/src-tauri/database-tree/postgresql.pin",
];
const IDENTITY_VARIABLE: &str = "SIGN_IDENTITY";

pub fn work_folder(clone: &Path) -> Result<PathBuf, BuilderError> {
    clone
        .parent()
        .map(|parent| parent.join(WORK))
        .ok_or(BuilderError::Bundle)
}

/// An empty or missing macOS identity is refused: the recipe would make an ad hoc tree.
pub fn build(
    clone: &Path,
    target: Target,
    work: &Path,
    identity: Option<&str>,
) -> Result<Command, BuilderError> {
    if target == Target::WindowsX86_64 {
        let mut step = command(
            Program::Bun,
            clone,
            &["apps/desktop/src-tauri/database-tree/windows.ts"],
        );
        step.args.push(work.as_os_str().to_owned());
        step.args.push(clone.join(OUT).into_os_string());
        return Ok(step);
    }
    let recipe_target = match target {
        Target::LinuxX86_64 => "linux",
        Target::WindowsX86_64 => unreachable!("Windows uses its native recipe"),
        Target::DarwinUniversal => "darwin-universal",
    };
    let mut sh = command(Program::Sh, clone, &[RECIPE, recipe_target]);
    sh.args.push(work.as_os_str().to_owned());
    sh.args.push(clone.join(OUT).into_os_string());
    if target == Target::DarwinUniversal {
        let identity = identity
            .filter(|name| !name.is_empty())
            .ok_or(BuilderError::Bundle)?;
        sh.env.push((IDENTITY_VARIABLE, identity.to_owned()));
    }
    Ok(sh)
}

/// Files the native Windows recipe reads, checked against the release commit.
pub const WINDOWS_COMMITTED: [&str; 3] = [
    "apps/desktop/src-tauri/database-tree/windows.ts",
    "apps/desktop/src-tauri/database-tree/windows.pin.json",
    "apps/desktop/src-tauri/database-tree/postgresql.pin",
];

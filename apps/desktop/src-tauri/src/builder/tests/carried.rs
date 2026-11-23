//! INS-002, S12 packaging.

mod link;
mod mach_o;
mod programs;
mod readings;
#[cfg(target_os = "macos")]
mod signed_app;
mod signed_files;
mod signer;
mod tree;

use crate::builder::plan::Command;

const STAGE: &str = "/c/apps/desktop/src-tauri/binaries";

fn args(command: &Command) -> Vec<&str> {
    command.args.iter().map(|a| a.to_str().unwrap()).collect()
}

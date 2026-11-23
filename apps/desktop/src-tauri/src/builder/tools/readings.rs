//! The tool check's OS readings (BLD-028, BLD-083); the decision on them is in `tools`.

use std::path::{Path, PathBuf};

use crate::builder::environment::Environment;

pub fn keychain_identities() -> Vec<String> {
    std::process::Command::new("/usr/bin/security")
        .args(["find-identity", "-v", "-p", "codesigning"])
        .output()
        .map_or_default(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(str::to_owned)
                .collect()
        })
}

/// Tauri keeps no bundle settings in the running app's config, so `tauri.conf.json` is read at compile time.
pub fn signing_identity() -> Option<String> {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../../../tauri.conf.json")).ok()?;
    config["bundle"]["macOS"]["signingIdentity"]
        .as_str()
        .map(str::to_owned)
}

/// Runs the steps' `rustc` in `folder`, where rustup reads a folder override; rustup may not install a toolchain.
pub fn rust_sysroot(env: &Environment, folder: &Path) -> Option<PathBuf> {
    let rustc = env.find("rustc", Path::is_file)?;
    let out = std::process::Command::new(rustc)
        .args(["--print", "sysroot"])
        .env_clear()
        .envs(env.for_step(&[("RUSTUP_AUTO_INSTALL", "0".to_owned())]))
        .current_dir(folder)
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    let line = text.lines().next()?.trim();
    (out.status.success() && Path::new(line).is_absolute()).then(|| PathBuf::from(line))
}

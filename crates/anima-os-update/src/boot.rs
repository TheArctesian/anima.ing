mod cutover;

use crate::{Error, Result, command, feed, state};
pub use anima_os_maintenance::cutover::BOOT_ENTRIES as ENTRIES;
use anima_os_maintenance::cutover::require_entry;
use std::{fs, path::Path};

pub fn current() -> Result<String> {
    let current = fs::canonicalize("/run/current-system")?;
    current
        .to_str()
        .map(str::to_owned)
        .ok_or(Error::Refused("The current system path is invalid."))
}

pub fn pin(name: &str, system: &str) -> Result<()> {
    let root = Path::new("/nix/var/nix/gcroots").join(format!("anima-{name}"));
    let temporary = root.with_extension("tmp");
    if temporary.symlink_metadata().is_ok() {
        fs::remove_file(&temporary)?;
    }
    std::os::unix::fs::symlink(system, &temporary)?;
    fs::rename(temporary, root)?;
    fs::File::open("/nix/var/nix/gcroots")?.sync_all()?;
    Ok(())
}

fn entry(system: &str, name: &str, title: &str, version: u8) -> Result<()> {
    if !feed::valid_system(system) {
        return Err(Error::Refused("The boot system path is invalid."));
    }
    let source = Path::new(system);
    let hash = source
        .file_name()
        .and_then(|s| s.to_str())
        .and_then(|s| s.split('-').next())
        .ok_or(Error::Refused("The boot system identity is invalid."))?;
    let destination = Path::new("/boot/EFI/anima").join(hash);
    fs::create_dir_all(&destination)?;
    for asset in ["kernel", "initrd"] {
        let temporary = destination.join(format!("{asset}.tmp"));
        fs::copy(source.join(asset), &temporary)?;
        fs::File::open(&temporary)?.sync_all()?;
        fs::rename(temporary, destination.join(asset))?;
    }
    fs::File::open(&destination)?.sync_all()?;
    let params = String::from_utf8(state::read_bounded(&source.join("kernel-params"), 65536)?)
        .map_err(|_| Error::Refused("The kernel parameters are invalid."))?;
    let body = format!(
        "title {title}\nsort-key zz-anima\nversion {version}\nlinux /EFI/anima/{hash}/kernel\ninitrd /EFI/anima/{hash}/initrd\noptions init={system}/init {}\n",
        params.trim()
    );
    state::atomic(&Path::new(ENTRIES).join(name), body.as_bytes())
}

pub async fn select(pattern: &str) -> Result<()> {
    state::atomic(
        Path::new("/boot/loader/loader.conf"),
        format!("default {pattern}\ntimeout 5\neditor no\n").as_bytes(),
    )?;
    command::run("bootctl", &["set-default", ""]).await?;
    Ok(())
}

pub async fn stage(system: &str) -> Result<String> {
    let current = current()?;
    if let Some(format) = cutover::required()? {
        cutover::generation(&current, format)?;
        cutover::generation(system, format)?;
    }
    pin("previous", &current)?;
    pin("candidate", system)?;
    entry(
        &current,
        "anima-current.conf",
        "Anima OS · previous system",
        1,
    )?;
    select("anima-current.conf").await?;
    for old in fs::read_dir(ENTRIES)? {
        let old = old?;
        let name = old.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("anima-candidate") && name.ends_with(".conf") {
            fs::remove_file(old.path())?;
        }
    }
    entry(
        system,
        "anima-candidate+2.conf",
        "Anima OS · preview update",
        2,
    )?;
    Ok(current)
}

pub fn check_activation(candidate: &str, previous: &str) -> Result<()> {
    let entries = Path::new(ENTRIES);
    let staged = entries.join("anima-candidate+2.conf");
    if !staged.is_file() {
        return Err(Error::Refused("No staged update is ready to restart."));
    }
    let format = cutover::required()?;
    require_entry(&staged, Some(candidate), format)?;
    cutover::fallback(entries, previous, format)
}

pub fn check_rollback(previous: &str) -> Result<()> {
    require_entry(
        &Path::new(ENTRIES).join("anima-current.conf"),
        Some(previous),
        cutover::required()?,
    )?;
    Ok(())
}

pub fn check_boot_retry(previous: &str) -> Result<()> {
    if let Some(format) = cutover::required()? {
        cutover::fallback(Path::new(ENTRIES), previous, Some(format))?;
    }
    Ok(())
}

pub fn check_current(system: &str) -> Result<()> {
    if let Some(format) = cutover::required()? {
        cutover::generation(system, format)?;
    }
    Ok(())
}

pub async fn activate() -> Result<()> {
    select("anima-*.conf").await?;
    reboot().await
}

pub async fn reboot() -> Result<()> {
    command::run("systemctl", &["--no-block", "reboot"])
        .await
        .map(drop)
}

pub async fn profile(system: &str) -> Result<()> {
    if !feed::valid_system(system) {
        return Err(Error::Refused("The retained system path is invalid."));
    }
    check_current(system)?;
    command::run(
        "nix-env",
        &["--profile", "/nix/var/nix/profiles/system", "--set", system],
    )
    .await
    .map(drop)
}

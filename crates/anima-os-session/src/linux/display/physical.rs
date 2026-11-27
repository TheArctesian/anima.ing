//! Resolve the current physical attachment for each display operation.
use crate::linux::{Error, Result, command};
use serde::Deserialize;
use std::{
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Attachment {
    #[serde(rename = "DISPLAY")]
    display: String,
    #[serde(rename = "XAUTHORITY")]
    authority: String,
    pid: u32,
    #[serde(rename = "bootId")]
    boot_id: String,
}

fn current() -> Result<Attachment> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open("/run/anima-console/physical.json")?;
    let admin = nix::unistd::User::from_name("admin")
        .map_err(std::io::Error::from)?
        .ok_or(Error::Refused(
            "The physical console account is unavailable.",
        ))?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.uid() != admin.uid.as_raw() || metadata.mode() & 0o077 != 0 {
        return Err(Error::Refused("Physical display state is not protected."));
    }
    let mut bytes = Vec::new();
    file.take(8193).read_to_end(&mut bytes)?;
    if bytes.len() > 8192 {
        return Err(Error::Refused("Physical display state exceeded its limit."));
    }
    let found: Attachment = serde_json::from_slice(&bytes)?;
    let local = found.display.strip_prefix(':').is_some_and(|value| {
        let parts: Vec<_> = value.split('.').collect();
        parts.len() <= 2
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()))
            && parts[0].parse::<u32>().is_ok_and(|number| number != 99)
    });
    if !local
        || found.display.len() > 32
        || !Path::new(&found.authority).is_absolute()
        || found.authority.len() > 4096
        || found.authority.chars().any(char::is_control)
        || found.pid == 0
        || found.boot_id != fs::read_to_string("/proc/sys/kernel/random/boot_id")?.trim()
        || fs::metadata(format!("/proc/{}", found.pid))?.uid() != admin.uid.as_raw()
    {
        return Err(Error::Refused(
            "Reconnect the physical display before changing its settings.",
        ));
    }
    Ok(found)
}

pub(super) async fn run(
    program: &str,
    arguments: &[&str],
    input: Option<&[u8]>,
    seconds: u64,
) -> Result<String> {
    let own_physical_session = !nix::unistd::Uid::effective().is_root()
        && std::env::var("ANIMA_SHARED_CONSOLE").as_deref() != Ok("1");
    if !matches!(program, "xrandr" | "xinput")
        || own_physical_session
        || !Path::new("/etc/anima/shared-console").try_exists()?
    {
        return command::run(program, arguments, input, seconds).await;
    }
    let attachment = current()?;
    let display = format!("DISPLAY={}", attachment.display);
    let authority = format!("XAUTHORITY={}", attachment.authority);
    let mut args = vec![display.as_str(), authority.as_str(), program];
    args.extend_from_slice(arguments);
    command::run("/run/current-system/sw/bin/env", &args, input, seconds).await
}

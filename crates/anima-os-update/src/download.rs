use crate::{
    Error, Result,
    feed::{self, Release},
    state,
};
use sha2::{Digest as _, Sha256};
use std::{
    fs::OpenOptions,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

pub const ARCHIVE: &str = "/persist/anima-updates/download.nar.xz";

pub async fn fetch(release: &Release) -> Result<()> {
    let mut output = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(ARCHIVE)?;
    let mut response = feed::response(&feed::client()?, &release.url).await?;
    let mut digest = Sha256::new();
    let mut count = 0_u64;
    let mut sampled = Instant::now();
    state::progress("downloading", Some(0), Some(release.bytes))?;
    while let Some(chunk) = tokio::time::timeout(Duration::from_secs(30), response.chunk())
        .await
        .map_err(|_| Error::Refused("The update download stalled."))??
    {
        count = count
            .checked_add(chunk.len() as u64)
            .ok_or(Error::Refused("The update exceeds its declared size."))?;
        if count > release.bytes {
            return Err(Error::Refused("The update exceeds its declared size."));
        }
        digest.update(&chunk);
        output.write_all(&chunk)?;
        if sampled.elapsed() >= Duration::from_millis(250) {
            state::progress("downloading", Some(count), Some(release.bytes))?;
            sampled = Instant::now();
        }
    }
    output.sync_all()?;
    state::progress("verifying", Some(count), Some(release.bytes))?;
    let actual = digest
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            use std::fmt::Write as _;
            let _ = write!(text, "{byte:02x}");
            text
        });
    if count != release.bytes || actual != release.sha256 {
        return Err(Error::Refused(
            "Downloaded update verification failed; the current system is unchanged.",
        ));
    }
    Ok(())
}

pub fn space(release: &Release) -> Result<()> {
    let free = |path: &str| {
        nix::sys::statvfs::statvfs(path)
            .map(|s| s.blocks_available().saturating_mul(s.fragment_size()))
            .map_err(|_| Error::Refused("Available update storage could not be read."))
    };
    let required = release.bytes + release.unpacked_bytes + 2 * 1024_u64.pow(3);
    if free("/nix")? < required || free("/boot")? < 512 * 1024_u64.pow(2) {
        return Err(Error::Refused(
            "Not enough free space to stage the update while retaining recovery.",
        ));
    }
    Ok(())
}

pub async fn import(release: &Release) -> Result<()> {
    state::progress("importing", Some(0), Some(release.unpacked_bytes))?;
    let mut unpack = Command::new("xz")
        .args([
            "--decompress",
            "--stdout",
            "--memlimit-decompress=1GiB",
            "--",
            ARCHIVE,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut import = Command::new("nix-store")
        .args(["--import", "--option", "require-sigs", "false"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let result = tokio::time::timeout(
        Duration::from_secs(3600),
        pump(&mut unpack, &mut import, release.unpacked_bytes),
    )
    .await
    .unwrap_or(Err(Error::Refused("The verified update import timed out.")));
    if result.is_err() {
        if unpack.try_wait()?.is_none() {
            unpack.kill().await?;
        }
        if import.try_wait()?.is_none() {
            import.kill().await?;
        }
    }
    result?;
    if !Path::new(&release.system).join("init").is_file()
        || !Path::new(&release.system)
            .join("etc/anima-preview")
            .is_file()
    {
        return Err(Error::Refused(
            "The imported system is not an Anima appliance.",
        ));
    }
    Ok(())
}

async fn pump(
    unpack: &mut tokio::process::Child,
    import: &mut tokio::process::Child,
    limit: u64,
) -> Result<()> {
    let mut input = unpack
        .stdout
        .take()
        .ok_or(Error::Refused("The update archive could not be read."))?;
    let mut output = import
        .stdin
        .take()
        .ok_or(Error::Refused("The verified update could not be imported."))?;
    let mut bytes = vec![0; 1024 * 1024];
    let mut total = 0_u64;
    let mut sampled = Instant::now();
    loop {
        let length = input.read(&mut bytes).await?;
        if length == 0 {
            break;
        }
        total += length as u64;
        if total > limit {
            return Err(Error::Refused(
                "The update unpacked beyond its declared limit.",
            ));
        }
        output.write_all(&bytes[..length]).await?;
        if sampled.elapsed() >= Duration::from_millis(250) {
            state::progress("importing", Some(total), Some(limit))?;
            sampled = Instant::now();
        }
    }
    drop(output);
    if !unpack.wait().await?.success() || !import.wait().await?.success() {
        return Err(Error::Refused("The verified system could not be imported."));
    }
    state::progress("importing", Some(total), Some(limit))
}

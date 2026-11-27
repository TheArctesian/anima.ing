//! Run bounded util-linux commands with the installed shell wrapper's tool path.
use std::{
    io::Read as _,
    process::{Command, Stdio},
};

use crate::{Error, Result};

pub(super) fn run(program: &'static str, args: &[&str]) -> Result<()> {
    execute(program, args, false, "120s").map(drop)
}

pub(super) fn capture(program: &'static str, args: &[&str], deadline: &str) -> Result<Vec<u8>> {
    execute(program, args, false, deadline)?.ok_or(Error::Command(program))
}

pub(super) fn optional_capture(program: &'static str, args: &[&str]) -> Result<Option<Vec<u8>>> {
    execute(program, args, true, "120s")
}

fn execute(
    program: &'static str,
    args: &[&str],
    absent: bool,
    deadline: &str,
) -> Result<Option<Vec<u8>>> {
    const MAX_BYTES: u64 = 4 * 1024 * 1024;

    let path = std::env::var_os("PATH").ok_or(Error::Command(program))?;
    let mut child = Command::new("timeout")
        .args(["--kill-after=5s", deadline, program])
        .args(args)
        .env_clear()
        .env("PATH", path)
        .env("LANG", "C")
        .env("HOME", "/root")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut bytes = Vec::new();
    let stdout = child.stdout.take().ok_or(Error::Command(program))?;
    let read = stdout.take(MAX_BYTES + 1).read_to_end(&mut bytes);
    let status = child.wait()?;
    read?;
    if absent && status.code() == Some(1) {
        return Ok(None);
    }
    if !status.success() || bytes.len() as u64 > MAX_BYTES {
        return Err(Error::Command(program));
    }
    Ok(Some(bytes))
}

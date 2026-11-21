use super::{Error, Result};
use std::{
    io::Read as _,
    os::unix::process::CommandExt as _,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn run(program: &'static str, arguments: &[&str]) -> Result<()> {
    execute(program, arguments, false, false).map(drop)
}

pub fn capture(program: &'static str, arguments: &[&str]) -> Result<Vec<u8>> {
    execute(program, arguments, true, false)?.ok_or(Error::Command(program))
}

pub fn optional_capture(program: &'static str, arguments: &[&str]) -> Result<Option<Vec<u8>>> {
    execute(program, arguments, true, true)
}

fn execute(
    program: &'static str,
    arguments: &[&str],
    capture: bool,
    absent: bool,
) -> Result<Option<Vec<u8>>> {
    let path = std::env::var_os("PATH").ok_or(Error::Command(program))?;
    let mut child = Command::new(program)
        .args(arguments)
        .env_clear()
        .env("PATH", path)
        .env("LANG", "C")
        .env("HOME", "/root")
        .stdin(Stdio::null())
        .stdout(if capture {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()?;
    let reader = child.stdout.take().map(|stdout| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stdout
                .take(4 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        })
    });
    let deadline = Instant::now()
        + Duration::from_secs(if program == "nixos-install" {
            1800
        } else {
            120
        });
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            if let Some(pid) =
                rustix::process::Pid::from_raw(i32::try_from(child.id()).unwrap_or(0))
            {
                let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Command(program));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let bytes = match reader {
        Some(reader) => reader.join().map_err(|_| Error::Command(program))??,
        None => Vec::new(),
    };
    if absent && status.code() == Some(1) {
        return Ok(None);
    }
    if !status.success() || bytes.len() > 4 * 1024 * 1024 {
        return Err(Error::Command(program));
    }
    Ok(Some(bytes))
}

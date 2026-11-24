use std::{process::Stdio, time::Duration};
use tokio::{io::AsyncWriteExt as _, process::Command};

use crate::{Error, Prompt};

pub(super) async fn install(prompt: Prompt, user: &str, existing: bool) -> Result<(), Error> {
    #[cfg(target_os = "linux")]
    let source = include_str!("linux.sh");
    #[cfg(target_os = "macos")]
    let source = include_str!("macos.sh");
    #[cfg(target_os = "windows")]
    let source = include_str!("windows.ps1");
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let source = "";
    run_source(prompt, &script(source, user, existing)).await
}

pub(super) async fn run_source(prompt: Prompt, source: &str) -> Result<(), Error> {
    let (mut command, input) = invocation(prompt, source)?;
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Error::Setup)?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|_| Error::Setup)?;
    }
    let status = tokio::time::timeout(Duration::from_mins(15), child.wait())
        .await
        .map_err(|_| Error::TimedOut)?
        .map_err(|_| Error::Setup)?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Setup)
    }
}

fn script(source: &str, user: &str, existing: bool) -> String {
    source
        .replace("ANIMA_LOGIN_USER", user)
        .replace("ANIMA_SSH_EXISTS", if existing { "yes" } else { "no" })
}

#[cfg(target_os = "linux")]
fn invocation(prompt: Prompt, source: &str) -> Result<(Command, String), Error> {
    let candidates = match prompt {
        Prompt::Desktop => ["/usr/bin/pkexec", "/run/current-system/sw/bin/pkexec"],
        Prompt::Terminal => ["/usr/bin/sudo", "/run/current-system/sw/bin/sudo"],
    };
    let program = candidates
        .into_iter()
        .find(|path| std::path::Path::new(path).is_file())
        .ok_or(Error::Setup)?;
    let mut command = Command::new(program);
    command.args(["/bin/sh", "-s"]);
    Ok((command, source.to_owned()))
}

#[cfg(target_os = "macos")]
fn invocation(prompt: Prompt, source: &str) -> Result<(Command, String), Error> {
    if !std::path::Path::new("/usr/sbin/sshd").is_file() {
        return Err(Error::Unsupported);
    }
    let script = source;
    if matches!(prompt, Prompt::Terminal) {
        let mut command = Command::new("/usr/bin/sudo");
        command.args(["/bin/sh", "-s"]);
        return Ok((command, script.to_owned()));
    }
    let escaped = script
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    Ok((
        Command::new("/usr/bin/osascript"),
        format!("do shell script \"{escaped}\" with administrator privileges\n"),
    ))
}

#[cfg(target_os = "windows")]
fn invocation(_prompt: Prompt, source: &str) -> Result<(Command, String), Error> {
    use base64::Engine as _;
    let bytes: Vec<u8> = source.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let root = std::env::var_os("SystemRoot").ok_or(Error::Unsupported)?;
    let program =
        std::path::PathBuf::from(root).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut command = Command::new(program);
    command.args(["-NoProfile", "-NonInteractive", "-Command", "-"]);
    let launcher = format!(
        "$ErrorActionPreference='Stop'; $p = Start-Process -FilePath \"$PSHOME\\powershell.exe\" -Verb RunAs -Wait -PassThru -ArgumentList '-NoProfile','-NonInteractive','-EncodedCommand','{encoded}'; exit $p.ExitCode\n"
    );
    Ok((command, launcher))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn invocation(_prompt: Prompt, _source: &str) -> Result<(Command, String), Error> {
    Err(Error::Unsupported)
}

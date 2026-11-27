//! Run one resolved Docker or Compose command without inheriting shell context.
mod capture;
pub(crate) use capture::{bounded as capture_bounded, program as capture_program, read as capture};

use std::{
    os::unix::process::CommandExt as _,
    path::Path,
    process::{Child, Command as Program, Stdio},
    thread,
    time::{Duration, Instant},
};

use rustix::process::{Pid, Signal, kill_process_group};

use super::Fault;
use crate::{Command, Paths};

pub(super) fn run(paths: &Paths, command: &Command, deadline: Instant) -> Result<(), Fault> {
    let mut child = prepared(paths, command)?
        .spawn()
        .map_err(|error| Fault::Spawn(error.kind()))?;
    fence(&mut child, deadline)
}

fn prepared(paths: &Paths, command: &Command) -> Result<Program, Fault> {
    if (command.program != paths.docker && command.program != paths.compose)
        || command.environment != paths.environment()
        || command
            .folder
            .as_ref()
            .is_some_and(|folder| !Path::new(folder).is_absolute())
    {
        return Err(Fault::UnsafePlan);
    }
    let mut process = Program::new(&command.program);
    process
        .args(&command.arguments)
        .env_clear()
        .envs(
            command
                .environment
                .iter()
                .map(|(name, value)| (name, value)),
        )
        .current_dir(command.folder.as_deref().unwrap_or("/"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    Ok(process)
}

fn fence(child: &mut Child, deadline: Instant) -> Result<(), Fault> {
    let interval = Duration::from_millis(20);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            terminate(child)?;
            return Err(Fault::LimitSeconds);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(Fault::CommandFailed)
                };
            }
            Ok(None) => thread::sleep(remaining.min(interval)),
            Err(error) => {
                terminate(child)?;
                return Err(Fault::Process(error.kind()));
            }
        }
    }
}

fn terminate(child: &mut Child) -> Result<(), Fault> {
    let pid = Pid::from_child(child);
    let signal = kill_process_group(pid, Signal::KILL);
    if let Err(error) = signal
        && error != rustix::io::Errno::SRCH
    {
        child.kill().map_err(|error| Fault::Process(error.kind()))?;
    }
    child.wait().map_err(|error| Fault::Process(error.kind()))?;
    Ok(())
}

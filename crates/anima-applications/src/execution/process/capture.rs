//! Bounded output for read-only startup probes.
use std::{
    io::{self, Read as _},
    process::{ChildStdout, Command as Program, Stdio},
    thread,
    time::{Duration, Instant},
};

use rustix::fs::{self, OFlags};

use super::{Command, Fault, Paths, fence, prepared, terminate};

pub(crate) fn read(paths: &Paths, command: &Command, deadline: Instant) -> Result<Vec<u8>, Fault> {
    bounded(paths, command, deadline, 1024)
}

pub(crate) fn bounded(
    paths: &Paths,
    command: &Command,
    deadline: Instant,
    limit: usize,
) -> Result<Vec<u8>, Fault> {
    program(prepared(paths, command)?, deadline, limit)
}

pub(crate) fn program(
    mut program: Program,
    deadline: Instant,
    limit: usize,
) -> Result<Vec<u8>, Fault> {
    if Instant::now() >= deadline {
        return Err(Fault::LimitSeconds);
    }
    let mut child = program
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| Fault::Spawn(error.kind()))?;
    let collected = child
        .stdout
        .take()
        .ok_or(Fault::Process(io::ErrorKind::BrokenPipe))
        .and_then(|output| collect(output, deadline, limit));
    let bytes = match collected {
        Ok(bytes) => bytes,
        Err(fault) => {
            terminate(&mut child)?;
            return Err(fault);
        }
    };
    fence(&mut child, deadline)?;
    Ok(bytes)
}

fn collect(mut output: ChildStdout, deadline: Instant, limit: usize) -> Result<Vec<u8>, Fault> {
    let mut buffer = [0; 1024];
    let mut bytes = Vec::new();
    let flags = fs::fcntl_getfl(&output)
        .map_err(std::io::Error::from)
        .map_err(|error| Fault::Process(error.kind()))?;
    fs::fcntl_setfl(&output, flags | OFlags::NONBLOCK)
        .map_err(std::io::Error::from)
        .map_err(|error| Fault::Process(error.kind()))?;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Fault::LimitSeconds);
        }
        match output.read(&mut buffer) {
            Ok(0) => return Ok(bytes),
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(remaining.min(Duration::from_millis(10)));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
            Err(error) => return Err(Fault::Process(error.kind())),
        }
        if bytes.len() > limit {
            return Err(Fault::Process(io::ErrorKind::InvalidData));
        }
    }
}

//! Read counters through a descriptor for the selected workload cgroup.
use nix::{
    fcntl::{OFlag, open, openat},
    sys::stat::Mode,
};
use serde::Serialize;
use std::{
    fs::File,
    io::Read as _,
    os::fd::OwnedFd,
    path::{Component, Path},
};

const AGENTS: &str = "/sys/fs/cgroup/anima.slice/anima-agents.slice";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::linux) struct Usage {
    cpu_micros: u64,
    memory_bytes: u64,
    tasks: u64,
}

pub(in crate::linux) fn read(scope: &str) -> Option<Usage> {
    let scope = Path::new(scope);
    let mut components = scope.components();
    let Some(Component::Normal(name)) = components.next() else {
        return None;
    };
    if components.next().is_some() {
        return None;
    }
    let directory = open(
        &Path::new(AGENTS).join(name),
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .ok()?;
    let tasks = contents(&directory, "pids.current")?.trim().parse().ok()?;
    let memory_bytes = contents(&directory, "memory.current")?
        .trim()
        .parse()
        .ok()?;
    let cpu = contents(&directory, "cpu.stat")?;
    let cpu_micros = cpu
        .lines()
        .find_map(|line| line.strip_prefix("usage_usec "))?
        .trim()
        .parse()
        .ok()?;
    let events = contents(&directory, "cgroup.events")?;
    (tasks > 0 && events.lines().any(|line| line == "populated 1")).then_some(Usage {
        cpu_micros,
        memory_bytes,
        tasks,
    })
}

fn contents(directory: &OwnedFd, counter: &str) -> Option<String> {
    let fd = openat(
        directory,
        counter,
        OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .ok()?;
    let mut value = String::new();
    File::from(fd).take(4097).read_to_string(&mut value).ok()?;
    (value.len() <= 4096).then_some(value)
}

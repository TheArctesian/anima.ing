use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};

#[derive(Deserialize, Serialize)]
pub struct Status {
    pub hostname: Option<String>,
    pub kernel: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub memory_available_bytes: Option<u64>,
    pub service_root_read_only: Option<bool>,
}

pub fn read() -> Status {
    let (total, available) = memory();
    Status {
        hostname: bounded_text("/proc/sys/kernel/hostname", 256),
        kernel: bounded_text("/proc/sys/kernel/osrelease", 256),
        uptime_seconds: uptime(),
        memory_total_bytes: total,
        memory_available_bytes: available,
        service_root_read_only: bounded_text("/proc/mounts", 262_144)
            .and_then(|text| root_protection(&text)),
    }
}

pub fn uptime() -> Option<u64> {
    bounded_text("/proc/uptime", 128)?
        .split_whitespace()
        .next()?
        .split('.')
        .next()?
        .parse()
        .ok()
}

pub fn memory() -> (Option<u64>, Option<u64>) {
    let Some(text) = bounded_text("/proc/meminfo", 16384) else {
        return (None, None);
    };
    (
        memory_bytes(&text, "MemTotal:"),
        memory_bytes(&text, "MemAvailable:"),
    )
}

pub fn bounded_text(path: impl AsRef<Path>, limit: u64) -> Option<String> {
    read_text(path.as_ref(), limit).ok()
}

pub fn read_text(path: &Path, limit: u64) -> io::Result<String> {
    let mut text = String::new();
    File::open(path)?
        .take(limit + 1)
        .read_to_string(&mut text)?;
    if u64::try_from(text.len()).unwrap_or(u64::MAX) > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "kernel reading exceeded its bound",
        ));
    }
    Ok(text.trim().to_owned())
}

fn memory_bytes(text: &str, key: &str) -> Option<u64> {
    let mut fields = text
        .lines()
        .find(|line| line.starts_with(key))?
        .split_whitespace();
    fields.next()?;
    let value: u64 = fields.next()?.parse().ok()?;
    (fields.next()? == "kB").then_some(value)?.checked_mul(1024)
}

fn root_protection(text: &str) -> Option<bool> {
    for line in text.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.get(1) == Some(&"/") {
            let options = fields.get(3)?;
            if options.split(',').any(|part| part == "ro") {
                return Some(true);
            }
            if options.split(',').any(|part| part == "rw") {
                return Some(false);
            }
            return None;
        }
    }
    None
}

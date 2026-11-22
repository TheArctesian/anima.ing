use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read};

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
    let memory = bounded_text("/proc/meminfo", 16384);
    Status {
        hostname: bounded_text("/proc/sys/kernel/hostname", 256),
        kernel: bounded_text("/proc/sys/kernel/osrelease", 256),
        uptime_seconds: bounded_text("/proc/uptime", 128)
            .and_then(|text| text.split('.').next()?.parse().ok()),
        memory_total_bytes: memory
            .as_deref()
            .and_then(|text| memory_bytes(text, "MemTotal:")),
        memory_available_bytes: memory
            .as_deref()
            .and_then(|text| memory_bytes(text, "MemAvailable:")),
        service_root_read_only: bounded_text("/proc/mounts", 262_144)
            .and_then(|text| root_protection(&text)),
    }
}

pub fn bounded_text(path: &str, limit: u64) -> Option<String> {
    let mut text = String::new();
    match File::open(path).and_then(|file| file.take(limit + 1).read_to_string(&mut text)) {
        Ok(_) if u64::try_from(text.len()).ok()? <= limit => Some(text.trim().to_owned()),
        Ok(_) | Err(_) => None,
    }
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

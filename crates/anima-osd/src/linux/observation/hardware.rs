//! Interprets CPU counters and the system battery entries exported by Linux.
use super::status::bounded_text;
use serde::Serialize;
use std::{fs, io, path::Path};

#[derive(Clone, Copy)]
pub(super) struct Cpu {
    total: u64,
    idle: u64,
}

impl Cpu {
    pub(super) fn from_stat(text: &str) -> Option<Self> {
        let mut fields = text.lines().next()?.split_whitespace();
        if fields.next()? != "cpu" {
            return None;
        }
        let mut counters = [0_u64; 8];
        for counter in &mut counters {
            *counter = fields.next()?.parse().ok()?;
        }
        Some(Self {
            total: counters.into_iter().try_fold(0_u64, u64::checked_add)?,
            idle: counters[3].checked_add(counters[4])?,
        })
    }

    pub(super) fn usage(self, previous: Self) -> Option<f64> {
        let total = self.total.checked_sub(previous.total)?;
        let idle = self.idle.checked_sub(previous.idle)?;
        let busy = total.checked_sub(idle)?;
        let hundredths = u128::from(busy)
            .checked_mul(10_000)?
            .checked_div(u128::from(total))?;
        Some(f64::from(u16::try_from(hundredths).ok()?) / 100.0)
    }
}

#[derive(Serialize)]
pub(super) struct Battery {
    name: String,
    percent: Option<u8>,
    state: String,
}

pub(super) fn batteries(directory: &Path) -> Option<Vec<Battery>> {
    let supply = match fs::read_dir(directory) {
        Ok(supply) => supply,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Some(Vec::new()),
        Err(_) => return None,
    };
    let mut batteries = Vec::new();
    for (index, entry) in supply.enumerate().take(65) {
        if index == 64 {
            return None;
        }
        let entry = entry.ok()?;
        if let Some(battery) = battery(&entry.path()) {
            batteries.push(battery);
        }
    }
    batteries.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    Some(batteries)
}

fn battery(path: &Path) -> Option<Battery> {
    let read = |field| bounded_text(path.join(field), 32);
    if read("type").as_deref() != Some("Battery")
        || read("scope").as_deref() == Some("Device")
        || read("present").as_deref() == Some("0")
    {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    if name.len() > 80 {
        return None;
    }
    Some(Battery {
        name: name.to_owned(),
        percent: read("capacity")
            .and_then(|value| value.parse::<u8>().ok())
            .filter(|value| *value <= 100),
        state: read("status")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Unknown".to_owned()),
    })
}

#[cfg(test)]
mod tests;

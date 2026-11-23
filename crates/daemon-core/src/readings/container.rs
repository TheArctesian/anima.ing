//! Readers of a container's cgroup v2 files (HMD-071): what the container of a hosted hub may use
//! and what it used, never a figure of the server that runs it (HMD-074).

use super::{Cpu, Memory, lines};

/// Thousandths of a processor, the unit of [`allowed_processors`].
pub const MILLI: u64 = 1000;

/// The use is `memory.current` less the file cache that the kernel can take back at once; `None`
/// when `memory.max` is `max`, as the total of the server does not stand in for the container's.
#[must_use]
pub fn memory(max: &[u8], current: &[u8], stat: &[u8]) -> Option<Memory> {
    let total_bytes = first_number(max).filter(|total| *total > 0)?;
    let current = first_number(current)?;
    let reclaimable = keyed(stat, "inactive_file").unwrap_or(0);
    Some(Memory {
        total_bytes,
        used_bytes: current.saturating_sub(reclaimable),
    })
}

#[must_use]
pub fn cpu_usage_micros(stat: &[u8]) -> Option<u64> {
    keyed(stat, "usage_usec")
}

/// The hard limit of `cpu.max` when it is under `processors`, else `processors`; `cpu.weight`
/// is a share under load, not a limit, and is not read.
#[must_use]
pub fn allowed_processors(cpu_max: &[u8], processors: u64) -> u64 {
    let all = processors.max(1).saturating_mul(MILLI);
    let quota = lines(cpu_max).next().and_then(|line| {
        let mut words = line.split_whitespace();
        let quota = words.next()?.parse::<u64>().ok()?;
        let period = words
            .next()?
            .parse::<u64>()
            .ok()
            .filter(|period| *period > 0)?;
        Some(quota.saturating_mul(MILLI) / period)
    });
    quota
        .filter(|quota| *quota > 0)
        .map_or(all, |quota| quota.min(all))
}

/// The time that the allowed processors had over `elapsed_micros`, and the part of it that the
/// container did not use.
#[must_use]
pub fn cpu_ticks(used_micros: u64, elapsed_micros: u64, allowed_milli: u64) -> Cpu {
    let total = elapsed_micros.saturating_mul(allowed_milli);
    let busy = used_micros.saturating_mul(MILLI);
    Cpu::Ticks {
        idle: total.saturating_sub(busy),
        total,
    }
}

fn first_number(output: &[u8]) -> Option<u64> {
    lines(output)
        .next()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

fn keyed(output: &[u8], key: &str) -> Option<u64> {
    lines(output).find_map(|line| {
        let mut words = line.split_whitespace();
        (words.next()? == key).then(|| words.next()?.parse().ok())?
    })
}

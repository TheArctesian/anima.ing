use std::time::Duration;

use super::{ThermalState, UsableMemory, UsableSource};
use crate::readings::GpuVendor;

const MIB: u64 = 1024 * 1024;

/// Above this much memory, macOS lets the GPU wire three quarters of it by default; at or below
/// it, two thirds.
const LARGE_MEMORY: u64 = 36 * 1024 * MIB;

pub const ACTIVE_WITHIN: Duration = Duration::from_mins(5);

#[must_use]
pub fn sysctl_number(output: &[u8]) -> Option<u64> {
    let text = std::str::from_utf8(output).ok()?;
    text.lines().next()?.trim().parse().ok()
}

/// The default share is the `recommendedMaxWorkingSetSize` that Metal gives on these machines;
/// the daemon does not call Metal (CAP-005).
#[must_use]
pub fn usable_memory(memory_bytes: u64, wired_limit_mb: Option<u64>) -> UsableMemory {
    if let Some(limit) = wired_limit_mb.filter(|limit| *limit > 0) {
        return UsableMemory {
            bytes: limit.saturating_mul(MIB).min(memory_bytes),
            source: UsableSource::WiredLimit,
        };
    }
    let bytes = if memory_bytes > LARGE_MEMORY {
        memory_bytes / 4 * 3
    } else {
        memory_bytes / 3 * 2
    };
    UsableMemory {
        bytes,
        source: UsableSource::DefaultShare,
    }
}

#[must_use]
pub fn pmset_thermal(output: &[u8]) -> Option<ThermalState> {
    let text = std::str::from_utf8(output).ok()?;
    let limit = text.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "CPU_Speed_Limit").then(|| value.trim().parse::<u32>().ok())?
    });
    match limit {
        Some(limit) if limit < 100 => Some(ThermalState::Throttled),
        Some(_) => Some(ThermalState::Nominal),
        None if text.contains("No thermal warning level has been recorded") => {
            Some(ThermalState::Nominal)
        }
        None => None,
    }
}

/// Reads `ioreg -c IOHIDSystem -d 4 -r -k HIDIdleTime`.
#[must_use]
pub fn ioreg_active(output: &[u8]) -> Option<bool> {
    let text = std::str::from_utf8(output).ok()?;
    let idle: u64 = text.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        key.trim_start_matches(|c: char| c == '|' || c.is_whitespace())
            .trim()
            .eq("\"HIDIdleTime\"")
            .then(|| value.trim().parse().ok())?
    })?;
    Some(Duration::from_nanos(idle) < ACTIVE_WITHIN)
}

/// The vendor of a PCI vendor ID as `/sys/class/drm/cardN/device/vendor` holds it (`0x10de`).
#[must_use]
pub fn pci_vendor(text: &str) -> Option<GpuVendor> {
    match text.trim() {
        "0x10de" => Some(GpuVendor::Nvidia),
        "0x1002" => Some(GpuVendor::Amd),
        "0x8086" => Some(GpuVendor::Intel),
        _ => None,
    }
}

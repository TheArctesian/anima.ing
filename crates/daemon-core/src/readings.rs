//! Readers of device command output (MET-002); none panics and none fails on one bad line.
//! A last line without its newline is dropped: the link cuts output at a limit, and half a
//! line can hold a wrong number.

pub mod apfs;
pub mod batteries;
#[cfg(any(test, feature = "test-support"))]
pub mod captures;
pub mod container;
pub mod df;
pub mod drives;
pub mod gpu;
pub mod linux;
pub mod macos;
mod reading;
#[cfg(test)]
mod tests;

pub use batteries::{Battery, BatteryState};
pub use drives::{Drive, DriveKind};
pub use gpu::{GpuReading, GpuVendor, Vram};
pub use reading::{Cpu, Disk, DriveCounters, HubStorage, Interface, Load, Memory, Reading};

const LABEL_LIMIT: usize = 128;

/// The complete lines of `output`, each `None` when it is not UTF-8, so positions hold.
fn positional_lines(output: &[u8]) -> impl Iterator<Item = Option<&str>> {
    let complete = output
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |end| end + 1);
    output
        .get(..complete)
        .unwrap_or_default()
        .split_inclusive(|byte| *byte == b'\n')
        .map(|line| std::str::from_utf8(line).ok())
}

fn lines(output: &[u8]) -> impl Iterator<Item = &str> {
    positional_lines(output).flatten()
}

fn number(text: &str) -> Option<f64> {
    text.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && !value.is_sign_negative())
}

fn label(text: &str) -> Option<&str> {
    let usable = !text.is_empty()
        && text.len() <= LABEL_LIMIT
        && !text
            .chars()
            .any(|c| c.is_control() || c == char::REPLACEMENT_CHARACTER);
    usable.then_some(text)
}

fn load<'a>(mut words: impl Iterator<Item = &'a str>) -> Option<Load> {
    Some(Load {
        one: number(words.next()?)?,
        five: number(words.next()?)?,
        fifteen: number(words.next()?)?,
    })
}

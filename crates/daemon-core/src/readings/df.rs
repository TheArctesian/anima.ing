//! Reader of `df -P` (MET-002).

use super::{Disk, label, lines};

/// One [`Disk`] per line of `df -P`; `block_bytes` is 1 for `-B1` (Linux), 1024 for `-k` (macOS).
#[must_use]
pub fn disks(output: &[u8], block_bytes: u64) -> Vec<Disk> {
    lines(output)
        .filter_map(|line| {
            let ([filesystem, total, used, available, capacity], mount) = columns(line)?;
            if !capacity.ends_with('%') {
                return None;
            }
            Some(Disk {
                filesystem: filesystem.to_owned(),
                mount: label(mount)?.to_owned(),
                total_bytes: total.parse::<u64>().ok()?.checked_mul(block_bytes)?,
                used_bytes: used.parse::<u64>().ok()?.checked_mul(block_bytes)?,
                available_bytes: available.parse::<u64>().ok()?.checked_mul(block_bytes)?,
            })
        })
        .collect()
}

fn columns(line: &str) -> Option<([&str; 5], &str)> {
    let mut rest = line;
    let mut words = [""; 5];
    for word in &mut words {
        let (head, tail) = rest.trim_start().split_once(char::is_whitespace)?;
        *word = head;
        rest = tail;
    }
    Some((words, rest.trim()))
}

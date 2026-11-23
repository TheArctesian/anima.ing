//! Reader of `nvidia-smi` (MET-016).

use super::{GpuReading, GpuVendor, vram};
use crate::readings::{label, lines, number};

const FALLBACK_NAME: &str = "NVIDIA GPU";

const MEBIBYTE: u64 = 1024 * 1024;

/// Output of `nvidia-smi --query-gpu=index,name,utilization.gpu,memory.used,memory.total,
/// temperature.gpu,power.draw --format=csv,noheader,nounits`. A name may hold commas.
#[must_use]
pub fn gpus(output: &[u8]) -> Vec<GpuReading> {
    lines(output).filter_map(gpu).collect()
}

fn gpu(line: &str) -> Option<GpuReading> {
    let mut fields = line.rsplitn(6, ',');
    let power = fields.next()?;
    let temperature = fields.next()?;
    let memory_total = fields.next()?;
    let memory_used = fields.next()?;
    let busy = fields.next()?;
    let (index, name) = fields.next()?.split_once(',')?;
    Some(GpuReading {
        index: index.trim().parse().ok()?,
        name: label(name.trim()).unwrap_or(FALLBACK_NAME).to_owned(),
        vendor: GpuVendor::Nvidia,
        busy_percent: number(busy.trim()),
        vram: vram(mebibytes(memory_used), mebibytes(memory_total)),
        temperature_c: number(temperature.trim()),
        power_w: number(power.trim()),
        shared_memory_bytes: None,
    })
}

fn mebibytes(field: &str) -> Option<u64> {
    field.trim().parse::<u64>().ok()?.checked_mul(MEBIBYTE)
}

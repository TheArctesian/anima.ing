//! The samples of each known GPU from one collection's answers (MET-006, MET-007, MET-009); pure.

use daemon_core::{points::Kind, readings::GpuReading};

use super::{
    answers::Answer,
    discovery::{Discovered, Found},
};
use crate::fleet::metrics::{GpuVendor, Sample};

/// A missing answer or value gives no sample, never zero; a GPU not in `known` waits for the next discovery (MET-007).
pub(in crate::fleet::metrics::collector) fn samples(
    known: &Discovered,
    answers: &[(GpuVendor, Answer)],
) -> Vec<Sample> {
    let mut out = Vec::new();
    for found in &known.gpus {
        if let Some(reading) = reading_of(found, answers) {
            add(&mut out, found, reading);
        }
    }
    out
}

fn reading_of<'a>(found: &Found, answers: &'a [(GpuVendor, Answer)]) -> Option<&'a GpuReading> {
    answers
        .iter()
        .filter(|(vendor, _)| *vendor == found.gpu.vendor)
        .find_map(|(_, answer)| match answer {
            Answer::Gpus(readings) => readings
                .iter()
                .find(|reading| reading.index == found.source),
            Answer::Failed | Answer::Unknown => None,
        })
}

/// A device number outside the K8 range gives none; discovery never gives one.
fn add(out: &mut Vec<Sample>, found: &Found, reading: &GpuReading) {
    let Ok(index) = u8::try_from(found.gpu.index) else {
        return;
    };
    let sample = |kind, value| Sample::gpu(kind, index, value);
    out.extend(
        reading
            .busy_percent
            .and_then(|busy| sample(Kind::GpuPercent, busy)),
    );
    if let Some(vram) = reading.vram.filter(|_| found.gpu.has_vram) {
        let used = vram.used_bytes.min(vram.total_bytes);
        let share = float(used) / float(vram.total_bytes) * 100.0;
        out.extend(sample(Kind::VramPercent, share));
        out.extend(sample(Kind::VramBytes, float(used)));
    }
    out.extend(
        reading
            .temperature_c
            .and_then(|celsius| sample(Kind::GpuTemperatureC, celsius)),
    );
    out.extend(
        reading
            .power_w
            .and_then(|watts| sample(Kind::GpuPowerW, watts)),
    );
}

#[expect(
    clippy::cast_precision_loss,
    reason = "byte counts above 2^53 lose their last digits, which a chart cannot show"
)]
const fn float(count: u64) -> f64 {
    count as f64
}

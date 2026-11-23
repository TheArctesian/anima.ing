//! Reader of `intel_gpu_top -J` (MET-016).

use serde_json::Value;

use super::{GpuReading, GpuVendor};

/// Intel's JSON does not name the device in every version, so the reading has this name.
const NAME: &str = "Intel GPU";

/// The last complete sample with a usable engine, as GPU 0; older versions omit the array brackets.
/// An integrated GPU shares system memory, so there is no VRAM (MET-007).
#[must_use]
pub fn gpu(output: &[u8]) -> Option<GpuReading> {
    let text = String::from_utf8_lossy(output);
    let sample = objects(&text)
        .filter_map(|object| serde_json::from_str::<Value>(object).ok())
        .filter_map(|sample| busy(&sample).map(|busy| (busy, power(&sample))))
        .last()?;
    let (busy_percent, power_w) = sample;
    Some(GpuReading {
        index: 0,
        name: NAME.to_owned(),
        vendor: GpuVendor::Intel,
        busy_percent: Some(busy_percent),
        vram: None,
        temperature_c: None,
        power_w,
        shared_memory_bytes: None,
    })
}

fn busy(sample: &Value) -> Option<f64> {
    sample
        .get("engines")?
        .as_object()?
        .values()
        .filter_map(|engine| engine.get("busy")?.as_f64())
        .filter(|busy| *busy >= 0.0)
        .reduce(f64::max)
}

fn power(sample: &Value) -> Option<f64> {
    sample
        .get("power")?
        .get("GPU")?
        .as_f64()
        .filter(|watts| *watts >= 0.0)
}

fn objects(text: &str) -> impl Iterator<Item = &str> {
    let (mut depth, mut start, mut in_string, mut escaped) = (0_usize, 0, false, false);
    text.char_indices().filter_map(move |(at, c)| {
        if in_string {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            return None;
        }
        match c {
            '"' if depth > 0 => in_string = true,
            '{' => {
                if depth == 0 {
                    start = at;
                }
                depth += 1;
            }
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    return text.get(start..=at);
                }
            }
            _ => {}
        }
        None
    })
}

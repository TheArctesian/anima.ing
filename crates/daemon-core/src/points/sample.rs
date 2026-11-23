//! The sample a collection produces (MET-002, MET-009, D-5, R-099).

use super::{Kind, MAX_GPU_INDEX};

/// No working GPU reads above this, so a higher value is a broken or hostile reading.
const MAX_GPU_TEMPERATURE_C: f64 = 1000.0;

/// Ten times any one GPU board, so a higher value is a broken or hostile reading.
const MAX_GPU_POWER_W: f64 = 10_000.0;

/// One value of one series at one collection: the label is a mount point, an interface name
/// or a GPU index, and empty for the kinds that have none (D-5).
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub kind: Kind,
    pub label: String,
    pub value: f64,
    /// Also the label; `None` for a kind that belongs to no GPU (MET-009, R-099).
    pub gpu_index: Option<u8>,
}

impl Sample {
    /// A sample for a kind that belongs to no GPU; `None` for a GPU kind (R-099) or a value
    /// that [`kept_value`] does not keep.
    #[must_use]
    pub fn new(kind: Kind, label: &str, value: f64) -> Option<Self> {
        if kind.is_per_gpu() {
            return None;
        }
        Some(Self {
            kind,
            label: label.to_owned(),
            value: kept_value(kind, value)?,
            gpu_index: None,
        })
    }

    /// A sample labelled with its GPU's index, so each GPU has its own series (MET-009, R-099);
    /// `None` for a kind that belongs to no GPU, an index above [`MAX_GPU_INDEX`] (K8 Section 4)
    /// or a value that [`kept_value`] does not keep.
    #[must_use]
    pub fn gpu(kind: Kind, gpu_index: u8, value: f64) -> Option<Self> {
        if !kind.is_per_gpu() || gpu_index > MAX_GPU_INDEX {
            return None;
        }
        Some(Self {
            kind,
            label: gpu_index.to_string(),
            value: kept_value(kind, value)?,
            gpu_index: Some(gpu_index),
        })
    }
}

#[must_use]
pub const fn ceiling(kind: Kind) -> f64 {
    match kind {
        Kind::GpuTemperatureC => MAX_GPU_TEMPERATURE_C,
        Kind::GpuPowerW => MAX_GPU_POWER_W,
        _ => f64::INFINITY,
    }
}

/// An SSH collection, the daemon's window and a pushed batch share this rule (MET-002, CR-17).
pub(crate) fn kept_value(kind: Kind, value: f64) -> Option<f64> {
    if !value.is_finite() || value.is_sign_negative() || value > ceiling(kind) {
        return None;
    }
    Some(if kind.is_percent() {
        value.min(100.0)
    } else {
        value
    })
}

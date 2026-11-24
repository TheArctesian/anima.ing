//! The GPUs of one collection (MET-006 to MET-009, D-30).
//! Only `reads` uses the link; the other modules are pure.

pub(super) mod answers;
pub(super) mod discovery;
mod reads;
pub(super) mod samples;
pub(super) mod sysfs;

use chrono::{DateTime, Utc};

pub(super) use answers::Answer;
pub(super) use discovery::Discovered;
pub(super) use reads::ask;

use crate::fleet::{
    devices::OperatingSystem,
    metrics::{
        GpuVendor, Reading, Sample,
        commands::{GpuCommand, gpu_commands},
    },
};
use discovery::Look;

pub(super) struct Plan {
    look: Option<Look>,
    known: Option<Discovered>,
    commands: Vec<GpuCommand>,
}

#[derive(Debug)]
pub(super) struct GpuRound {
    pub(super) known: Option<Discovered>,
    /// This collection looked, so `known` is to be stored.
    pub(super) discovered_now: bool,
    pub(super) samples: Vec<Sample>,
}

impl Plan {
    pub(super) fn new(
        os: OperatingSystem,
        known: Option<Discovered>,
        reading: &Reading,
        at: DateTime<Utc>,
    ) -> Self {
        let look = discovery::due(known.as_ref(), discovery::boot_at(reading, at), at);
        let vendors: Vec<GpuVendor> = known
            .iter()
            .flat_map(|known| &known.gpus)
            .map(|found| found.gpu.vendor)
            .collect();
        let mut commands = gpu_commands(os);
        commands.retain(|command| {
            vendors.contains(&command.vendor)
                || look
                    .as_ref()
                    .is_some_and(|look| look.includes(command.vendor))
        });
        Self {
            look,
            known,
            commands,
        }
    }

    pub(super) fn commands(&self) -> &[GpuCommand] {
        &self.commands
    }

    pub(super) fn finish(self, answers: &[(GpuVendor, Answer)]) -> GpuRound {
        let discovered_now = self.look.is_some();
        let known = match &self.look {
            Some(look) => Some(look.merge(self.known.as_ref(), answers)),
            None => self.known,
        };
        let samples = known
            .as_ref()
            .map_or_default(|known| samples::samples(known, answers));
        GpuRound {
            known,
            discovered_now,
            samples,
        }
    }
}

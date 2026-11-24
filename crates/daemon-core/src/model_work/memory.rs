//! The memory of a node for the loads of its copies (K11 Section 11.1, FIT-020 to FIT-025): the reservation of
//! a copy, the room for a load, the unload of idle copies and the return of a pin. Pure: the caller gives the
//! clock in seconds and does each load and unload.

pub mod keep;
pub mod returns;
pub mod room;
#[cfg(test)]
mod tests;

use uuid::Uuid;

/// A memory that the node counts its reservations against: the main memory, or a GPU with its own memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Memory {
    Main,
    Gpu(u8),
}

impl Memory {
    /// The word of the report (`main`, `gpu0`).
    #[must_use]
    pub fn word(self) -> String {
        match self {
            Self::Main => "main".to_owned(),
            Self::Gpu(index) => format!("gpu{index}"),
        }
    }
}

/// The state of a copy on a server (FIT-025).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnServer {
    Loading,
    Ready,
    Active,
    Unloading,
}

impl OnServer {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Active => "active",
            Self::Unloading => "unloading",
        }
    }
}

/// A copy that holds a reservation on the node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
    pub copy: Uuid,
    pub state: OnServer,
    /// The whole reservation, for the report.
    pub reserved: u64,
    /// The part of the reservation on each memory that it uses.
    pub parts: Parts,
    pub keep: keep::Keep,
    /// The second of the end of its last request, or of its load.
    pub used_at: u64,
    /// A return loaded it and no request used it since.
    pub returned_unused: bool,
}

/// The part of a reservation on each memory, each memory once.
pub type Parts = Vec<(Memory, u64)>;

/// The part of `parts` on `memory`, or 0.
#[must_use]
pub fn part_on(parts: &Parts, memory: Memory) -> u64 {
    parts
        .iter()
        .filter(|(on, _)| *on == memory)
        .fold(0, |sum, (_, bytes)| sum.saturating_add(*bytes))
}

/// The sum of the parts that count against `memory`.
#[must_use]
pub fn reserved_on(held: &[Held], memory: Memory) -> u64 {
    held.iter().fold(0, |sum, copy| {
        sum.saturating_add(part_on(&copy.parts, memory))
    })
}

/// The parts of a `llama_server` load (Section 11.1): `gpu_layers` of `layers` on the GPUs, in equal parts, and
/// the rest on main memory; with no GPU, all on main; with the layers unknown, the whole on each memory.
#[must_use]
pub fn llama_parts(reserved: u64, gpus: &[Memory], gpu_layers: GpuShare) -> Parts {
    if gpus.is_empty() {
        return vec![(Memory::Main, reserved)];
    }
    let on_gpus = match gpu_layers {
        GpuShare::All => reserved,
        GpuShare::Layers {
            on_gpu,
            of: Some(of),
        } if of > 0 => {
            let share = u128::from(reserved) * u128::from(on_gpu.min(of)) / u128::from(of);
            u64::try_from(share).unwrap_or(reserved)
        }
        GpuShare::Layers { .. } => {
            let mut parts: Parts = gpus.iter().map(|&gpu| (gpu, reserved)).collect();
            parts.insert(0, (Memory::Main, reserved));
            return parts;
        }
    };
    let count = u64::try_from(gpus.len()).unwrap_or(u64::MAX);
    let each = on_gpus.div_ceil(count);
    let mut parts: Parts = gpus.iter().map(|&gpu| (gpu, each)).collect();
    let rest = reserved.saturating_sub(on_gpus);
    if rest > 0 {
        parts.insert(0, (Memory::Main, rest));
    }
    parts
}

/// How much of a `llama_server` load is on the GPUs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuShare {
    All,
    /// `on_gpu` layers of `of`, when the daemon knows the layers of the copy.
    Layers {
        on_gpu: u32,
        of: Option<u32>,
    },
}

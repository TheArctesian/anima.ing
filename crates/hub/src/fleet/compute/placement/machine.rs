//! The fit of one variant on one machine (FIT-001, FIT-005; task L1-20): the size and the layer
//! shape from the library, the memory and the backends from the hardware values that the daemon
//! of the machine sent (CAP-001 to CAP-005), and the memory share of its device policy.

use daemon_core::hardware::{BackendName, Hardware};

use super::fit::{CONTEXT_STEP, FitError, FitRequest, FitVerdict, KvShape, estimate_fit};
use super::overhead::OverheadTable;
use crate::fleet::compute::library::{Facts, Format, Variant};

/// The context of an estimate when none is asked: a usual default of a runtime, or the context
/// of the model when it is smaller.
pub const DEFAULT_CONTEXT: u64 = 8_192;

/// The bytes of one element of the KV cache: f16, the default of llama.cpp.
const KV_ELEMENT_BYTES: u32 = 2;

/// The GPU backends in the order that a runtime takes them.
const GPU_BACKENDS: [BackendName; 4] = [
    BackendName::Cuda,
    BackendName::Rocm,
    BackendName::Metal,
    BackendName::Vulkan,
];

/// What the estimate knows of a machine.
#[derive(Clone, Copy, Debug)]
pub struct Machine<'a> {
    pub hardware: &'a Hardware,
    /// The memory share of the device policy, 1 to 100 percent (CAP-011).
    pub memory_share: u8,
    pub overheads: &'a OverheadTable,
}

/// Where the weights of the model are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// In the memory of the GPU, or the unified memory of Apple silicon.
    Gpu,
    /// Some layers in the video memory and the rest in the system memory: slower.
    Partial,
    /// In the system memory: the machine has no usable GPU backend.
    Cpu,
}

/// The verdict of one variant on one machine, with the request it answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Estimate {
    pub request: FitRequest,
    pub verdict: FitVerdict,
    pub backend: BackendName,
    pub placement: Placement,
}

/// Why there is no estimate.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Unknown {
    #[error("the model files do not give the layers, the hidden size and the attention heads")]
    Shape,
    #[error(transparent)]
    Fit(#[from] FitError),
}

/// The fit of `variant` of the model with `facts` on `machine`, at `context` tokens or the
/// default context.
///
/// # Errors
/// [`Unknown`] when the files do not give the layer shape, or a size overflows.
pub fn estimate(
    facts: &Facts,
    variant: &Variant,
    machine: Machine<'_>,
    context: Option<u64>,
) -> Result<Estimate, Unknown> {
    let kv = kv_shape(facts).ok_or(Unknown::Shape)?;
    let request = |backend: BackendName, memory: u64| FitRequest {
        weight_bytes: variant.size_bytes(),
        kv,
        context_tokens: context_of(facts, context),
        overhead_bytes: machine
            .overheads
            .bytes_for(runtime(variant.format), backend.as_str()),
        usable_bytes: share_of(memory, machine.memory_share),
    };
    let hardware = machine.hardware;
    let Some(gpu) = gpu_backend(hardware) else {
        let asked = request(BackendName::Cpu, hardware.system_memory_bytes);
        return Ok(answer(asked, BackendName::Cpu, Placement::Cpu)?);
    };
    let on_gpu = answer(request(gpu, gpu_memory(hardware, gpu)), gpu, Placement::Gpu)?;
    let dedicated = hardware
        .dedicated_memory_bytes
        .filter(|_| gpu != BackendName::Metal);
    match (on_gpu.verdict, dedicated) {
        (FitVerdict::Fits { .. }, _) | (_, None) => Ok(on_gpu),
        (_, Some(video)) => {
            let both = video.saturating_add(hardware.system_memory_bytes);
            Ok(answer(request(gpu, both), gpu, Placement::Partial)?)
        }
    }
}

/// `percent` of `memory`, 1 to 100 percent (CAP-011).
fn share_of(memory: u64, percent: u8) -> u64 {
    let part = u128::from(memory) * u128::from(percent.clamp(1, 100)) / 100;
    u64::try_from(part).unwrap_or(memory)
}

fn answer(
    request: FitRequest,
    backend: BackendName,
    placement: Placement,
) -> Result<Estimate, FitError> {
    Ok(Estimate {
        verdict: estimate_fit(&request)?,
        request,
        backend,
        placement,
    })
}

/// FIT-002: the head dimension is the hidden size over the attention heads; a model with no
/// grouped KV heads has as many KV heads as attention heads.
fn kv_shape(facts: &Facts) -> Option<KvShape> {
    let shape = facts.shape;
    let heads = shape.attention_heads.filter(|heads| *heads > 0)?;
    Some(KvShape {
        layers: shape.layers?,
        kv_heads: shape.key_value_heads.unwrap_or(heads),
        head_dim: shape.hidden_size? / heads,
        bytes_per_element: KV_ELEMENT_BYTES,
    })
}

/// The asked context, else the default; never more than the model has, in steps of 256 tokens.
fn context_of(facts: &Facts, asked: Option<u64>) -> u64 {
    let most = facts.context_length.map_or(u64::MAX, u64::from);
    let tokens = asked.unwrap_or(DEFAULT_CONTEXT).min(most);
    (tokens - tokens % CONTEXT_STEP).max(CONTEXT_STEP)
}

const fn runtime(format: Format) -> &'static str {
    match format {
        Format::Gguf => "llama.cpp",
        Format::Mlx => "mlx",
        Format::Safetensors => "vllm",
    }
}

fn gpu_backend(hardware: &Hardware) -> Option<BackendName> {
    GPU_BACKENDS.into_iter().find(|name| {
        hardware
            .backends
            .iter()
            .any(|backend| backend.name == *name && backend.is_usable())
    })
}

/// The memory of a GPU backend: the usable memory of Apple silicon (CAP-005), else the video
/// memory, else the system memory.
fn gpu_memory(hardware: &Hardware, gpu: BackendName) -> u64 {
    let unified = hardware
        .usable_memory
        .map(|usable| usable.bytes)
        .or(hardware.unified_memory_bytes);
    let own = if gpu == BackendName::Metal {
        unified
    } else {
        hardware.dedicated_memory_bytes
    };
    own.unwrap_or(hardware.system_memory_bytes)
}

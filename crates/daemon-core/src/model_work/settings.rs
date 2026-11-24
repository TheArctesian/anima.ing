//! The flag policy (Section 2.1, SEC-C-007, CAG-004): `settings` is typed members, never flags.

use serde_json::{Map, Value};

use super::cause::Cause;
use super::names::Runtime;

/// The daemon owns these values: a member with one of these names, or whose name starts with `-`, is refused with `flag_owned`.
pub const OWNED: [&str; 14] = [
    "host",
    "port",
    "apiKey",
    "parallel",
    "modelPath",
    "trustRemoteCode",
    "logPath",
    "chatTemplate",
    "chatTemplateFile",
    "environment",
    "env",
    "arguments",
    "flags",
    "command",
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GpuLayers {
    Count(u32),
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KvCache {
    F16,
    Q8,
    Q4,
}

impl KvCache {
    /// The word as the runtime and the hub spell it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::F16 => "f16",
            Self::Q8 => "q8_0",
            Self::Q4 => "q4_0",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerSettings {
    pub context_tokens: Option<u32>,
    pub gpu_layers: Option<GpuLayers>,
    pub memory_share: Option<f64>,
    pub gpu: Option<u32>,
    pub batch_tokens: Option<u32>,
    pub kv_cache: Option<KvCache>,
}

#[derive(Clone, Copy, Debug)]
pub struct SettingLimits<'a> {
    /// The memory share of the device policy (RTC-003), from 0 to 1.
    pub memory_share: f64,
    pub gpus: &'a [u32],
}

fn integer(value: &Value, low: u64, high: u64) -> Result<u32, Cause> {
    value
        .as_u64()
        .filter(|number| (low..=high).contains(number))
        .and_then(|number| u32::try_from(number).ok())
        .ok_or(Cause::MemberUnknown)
}

/// The runtimes that may get each member (the table of Section 2.1).
fn runtimes_of(name: &str) -> Option<&'static [Runtime]> {
    const ALL: &[Runtime] = &[Runtime::LlamaServer, Runtime::MlxLm, Runtime::Vllm];
    match name {
        "contextTokens" | "batchTokens" => Some(ALL),
        "gpuLayers" | "kvCacheType" => Some(&[Runtime::LlamaServer]),
        "memoryShare" | "gpu" => Some(&[Runtime::Vllm]),
        _ => None,
    }
}

fn set(
    settings: &mut ServerSettings,
    name: &str,
    value: &Value,
    limits: SettingLimits<'_>,
) -> Result<(), Cause> {
    match name {
        "contextTokens" => settings.context_tokens = Some(integer(value, 512, 1_048_576)?),
        "batchTokens" => settings.batch_tokens = Some(integer(value, 16, 65_536)?),
        "gpuLayers" => {
            settings.gpu_layers = Some(if value.as_str() == Some("all") {
                GpuLayers::All
            } else {
                GpuLayers::Count(integer(value, 0, 999)?)
            });
        }
        "kvCacheType" => {
            let kinds = [KvCache::F16, KvCache::Q8, KvCache::Q4];
            let kind = kinds
                .into_iter()
                .find(|kind| value.as_str() == Some(kind.as_str()));
            settings.kv_cache = Some(kind.ok_or(Cause::MemberUnknown)?);
        }
        "memoryShare" => {
            let share = value
                .as_f64()
                .filter(|share| (0.05..=limits.memory_share).contains(share));
            settings.memory_share = Some(share.ok_or(Cause::MemoryShare)?);
        }
        _ => {
            let index = integer(value, 0, 255)?;
            if !limits.gpus.contains(&index) {
                return Err(Cause::GpuUnknown);
            }
            settings.gpu = Some(index);
        }
    }
    Ok(())
}

/// The members of `settings` as the hub sends them; [`read`] reads them back to the same settings.
#[must_use]
pub fn to_json(settings: &ServerSettings) -> Map<String, Value> {
    let mut object = Map::new();
    let mut put = |name: &str, value: Value| {
        object.insert(name.to_owned(), value);
    };
    if let Some(tokens) = settings.context_tokens {
        put("contextTokens", tokens.into());
    }
    if let Some(tokens) = settings.batch_tokens {
        put("batchTokens", tokens.into());
    }
    match settings.gpu_layers {
        Some(GpuLayers::Count(layers)) => put("gpuLayers", layers.into()),
        Some(GpuLayers::All) => put("gpuLayers", "all".into()),
        None => {}
    }
    if let Some(kind) = settings.kv_cache {
        put("kvCacheType", kind.as_str().into());
    }
    if let Some(share) = settings.memory_share {
        put("memoryShare", share.into());
    }
    if let Some(gpu) = settings.gpu {
        put("gpu", gpu.into());
    }
    object
}

/// With no runtime (a `model_load`), a member is permitted when one runtime permits it.
///
/// # Errors
/// A [`Cause`] for a member the daemon owns, an unknown member or a value out of range.
pub fn read(
    object: &Map<String, Value>,
    runtime: Option<Runtime>,
    limits: SettingLimits<'_>,
) -> Result<ServerSettings, Cause> {
    if object
        .keys()
        .any(|name| OWNED.contains(&name.as_str()) || name.starts_with('-'))
    {
        return Err(Cause::FlagOwned);
    }
    let mut settings = ServerSettings::default();
    for (name, value) in object {
        let runtimes = runtimes_of(name).ok_or(Cause::MemberUnknown)?;
        if runtime.is_some_and(|runtime| !runtimes.contains(&runtime)) {
            return Err(Cause::MemberUnknown);
        }
        set(&mut settings, name, value, limits)?;
    }
    Ok(settings)
}

//! A runtime's command line and environment (Section 2.1, SEC-C-001, SEC-C-007); no word of the hub reaches the command line but the numbers of [`ServerSettings`].

use std::path::Path;

use super::settings::{GpuLayers, ServerSettings};

/// The variable that holds the API key of a `llama_server` (never on its command line).
pub const LLAMA_KEY_VARIABLE: &str = "LLAMA_API_KEY";

pub const PARALLEL_SLOTS: u32 = 1;

/// The numbers of [`ServerSettings`] as `llama_server` names them, with no dashes: the keys of a preset
/// (K11 Section 11.1) and, with `--`, the flags of a command line.
#[must_use]
pub fn llama_settings(settings: &ServerSettings) -> Vec<(&'static str, String)> {
    let mut pairs = Vec::new();
    if let Some(tokens) = settings.context_tokens {
        pairs.push(("ctx-size", tokens.to_string()));
    }
    if let Some(tokens) = settings.batch_tokens {
        pairs.push(("batch-size", tokens.to_string()));
    }
    match settings.gpu_layers {
        Some(GpuLayers::Count(layers)) => pairs.push(("n-gpu-layers", layers.to_string())),
        Some(GpuLayers::All) => pairs.push(("n-gpu-layers", "999".to_owned())),
        None => {}
    }
    if let Some(kind) = settings.kv_cache {
        pairs.push(("cache-type-k", kind.as_str().to_owned()));
        pairs.push(("cache-type-v", kind.as_str().to_owned()));
    }
    pairs
}

/// A `llama_server` in router mode (RTC-005) on `127.0.0.1:<port>`, with the copies of `preset` and no load of
/// its own: the daemon loads and unloads (K11 Section 11.1). `--metrics` gives the daemon the use of each copy
/// (counts only, no prompt).
#[must_use]
pub fn llama_arguments(settings: &ServerSettings, port: u16, preset: &Path) -> Vec<String> {
    let mut words: Vec<String> = [
        "--host",
        "127.0.0.1",
        "--port",
        &port.to_string(),
        "--log-disable",
        "--verbosity",
        "0",
        "--no-slots",
        "--parallel",
        &PARALLEL_SLOTS.to_string(),
        "--models-preset",
        &preset.display().to_string(),
        "--no-models-autoload",
        "--models-max",
        "0",
        "--metrics",
    ]
    .map(str::to_owned)
    .to_vec();
    for (name, value) in llama_settings(settings) {
        words.extend([format!("--{name}"), value]);
    }
    words
}

/// The whole environment of a runtime: nothing else, so no proxy, no token, no `DYLD_*` and no `LD_PRELOAD`.
#[must_use]
pub fn environment(temporary: &Path, bin: &Path, key: &str) -> Vec<(String, String)> {
    let temporary = temporary.display().to_string();
    vec![
        ("HOME".to_owned(), temporary.clone()),
        ("TMPDIR".to_owned(), temporary),
        ("PATH".to_owned(), bin.display().to_string()),
        ("HF_HUB_OFFLINE".to_owned(), "1".to_owned()),
        ("TRANSFORMERS_OFFLINE".to_owned(), "1".to_owned()),
        ("VLLM_NO_USAGE_STATS".to_owned(), "1".to_owned()),
        ("DO_NOT_TRACK".to_owned(), "1".to_owned()),
        (LLAMA_KEY_VARIABLE.to_owned(), key.to_owned()),
    ]
}

/// Never `--trust-remote-code`, only safetensors weights and no request logs (Section 2.1, SEC-C-008).
/// The API key is in the environment, never on the command line.
#[must_use]
pub fn vllm_arguments(copy: &Path, port: u16) -> Vec<String> {
    let copy = copy.display().to_string();
    [
        "serve",
        &copy,
        "--host",
        "127.0.0.1",
        "--port",
        &port.to_string(),
        "--load-format",
        "safetensors",
        "--tokenizer",
        &copy,
        "--disable-log-requests",
    ]
    .map(str::to_owned)
    .to_vec()
}

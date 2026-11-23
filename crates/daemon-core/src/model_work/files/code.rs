//! The checks of the files of a copy before a load (Section 2.2, SEC-C-008): a copy whose
//! `config.json` or `tokenizer_config.json` asks for remote code is refused, and a chat template
//! is at most 64 KiB. Pure: the daemon reads the bytes (at most [`MAX_CONFIG_BYTES`]) and gives
//! them here; nothing is run. A file that does not read as a JSON object is refused too, so
//! the check fails closed.

use serde_json::Value;

use super::classes;
use crate::model_work::cause::Cause;
use crate::model_work::names::Runtime;

/// The largest `config.json` or `tokenizer_config.json` that the daemon reads (2 MiB).
pub const MAX_CONFIG_BYTES: usize = 2 * 1024 * 1024;

/// The longest chat template of a copy, in bytes (Section 2.2).
pub const MAX_TEMPLATE_BYTES: u64 = 64 * 1024;

/// The members that make a runtime load code of the repository.
const REMOTE_CODE_MEMBERS: [&str; 2] = ["auto_map", "custom_pipelines"];

/// The JSON files of the Section 2.2 list that the daemon reads before a load. `tokenizer.json`
/// is not read: it holds no class or code member for these runtimes and can be tens of MiB.
pub const CHECKED_JSON: [&str; 5] = [
    "config.json",
    "generation_config.json",
    "tokenizer_config.json",
    "special_tokens_map.json",
    "preprocessor_config.json",
];

/// The files of one copy that the daemon read: each checked JSON file (and each
/// `*.safetensors.index.json`) as its name in the copy and its bytes, and the length of
/// `chat_template.jinja` when the copy has it.
#[derive(Clone, Copy, Debug, Default)]
pub struct CopyFiles<'a> {
    pub json: &'a [(&'a str, &'a [u8])],
    pub template_file: Option<u64>,
}

/// The checks of one copy for `runtime`: no remote-code member in any JSON file, only the
/// classes of the runtime (Section 2.2), and a chat template of at most 64 KiB.
///
/// # Errors
///
/// `remote_code` when a file asks for remote code or names a class that is not in the list of
/// the runtime, a file does not read or is too large, a name is not a checked JSON file, or a
/// chat template is longer than [`MAX_TEMPLATE_BYTES`].
pub fn check_copy(runtime: Runtime, files: &CopyFiles<'_>) -> Result<(), Cause> {
    let list = classes::architectures(runtime);
    for (name, bytes) in files.json {
        let base = name.rsplit('/').next().unwrap_or_default();
        match base {
            "config.json" => check_classes(&read_object(bytes)?, list, "architectures")?,
            "tokenizer_config.json" => {
                check_tokenizer_config(bytes)?;
                check_classes(&read_object(bytes)?, list, "tokenizer_class")?;
            }
            _ if CHECKED_JSON.contains(&base) || is_index(base) => check_config(bytes)?,
            _ => return Err(Cause::RemoteCode),
        }
    }
    files.template_file.map_or(Ok(()), check_template_length)
}

fn is_index(base: &str) -> bool {
    base.len() > ".safetensors.index.json".len() && base.ends_with(".safetensors.index.json")
}

/// `architectures` must be a list of permitted classes; `tokenizer_class`, when present, one
/// permitted class. With no list for the runtime, nothing is checked here.
fn check_classes(
    object: &serde_json::Map<String, Value>,
    list: Option<&[&str]>,
    member: &str,
) -> Result<(), Cause> {
    let Some(list) = list else { return Ok(()) };
    let allowed = |value: &Value| match value {
        Value::String(name) if member == "architectures" => classes::permitted(list, name),
        Value::String(name) => classes::permitted(&classes::TOKENIZER_CLASSES, name),
        _ => false,
    };
    let ok = match (member, object.get(member)) {
        ("architectures", Some(Value::Array(names))) => {
            !names.is_empty() && names.iter().all(allowed)
        }
        ("tokenizer_class", None) => true,
        ("tokenizer_class", Some(value)) => allowed(value),
        _ => false,
    };
    if ok { Ok(()) } else { Err(Cause::RemoteCode) }
}

/// Refuses a `config.json` that has the member `auto_map` or `custom_pipelines` at any depth.
///
/// # Errors
///
/// `remote_code` as for [`check_copy`].
pub fn check_config(bytes: &[u8]) -> Result<(), Cause> {
    read_object(bytes).map(|_| ())
}

/// [`check_config`], and the template of the member `chat_template` (a text, or a list of
/// `{name, template}`) is at most [`MAX_TEMPLATE_BYTES`] in total.
///
/// # Errors
///
/// `remote_code` as for [`check_copy`].
pub fn check_tokenizer_config(bytes: &[u8]) -> Result<(), Cause> {
    let object = read_object(bytes)?;
    match object.get("chat_template") {
        None | Some(Value::Null) => Ok(()),
        Some(Value::String(text)) => check_template_length(length(text)),
        Some(Value::Array(list)) => {
            let mut total = 0_u64;
            for entry in list {
                match entry.get("template") {
                    Some(Value::String(text)) => total = total.saturating_add(length(text)),
                    _ => return Err(Cause::RemoteCode),
                }
            }
            check_template_length(total)
        }
        Some(_) => Err(Cause::RemoteCode),
    }
}

/// Refuses a chat template (of a file, a JSON member or GGUF metadata) longer than 64 KiB.
///
/// # Errors
///
/// `remote_code` when `bytes` is larger than [`MAX_TEMPLATE_BYTES`].
pub fn check_template_length(bytes: u64) -> Result<(), Cause> {
    if bytes > MAX_TEMPLATE_BYTES {
        Err(Cause::RemoteCode)
    } else {
        Ok(())
    }
}

fn length(text: &str) -> u64 {
    u64::try_from(text.len()).unwrap_or(u64::MAX)
}

fn read_object(bytes: &[u8]) -> Result<serde_json::Map<String, Value>, Cause> {
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(Cause::RemoteCode);
    }
    match serde_json::from_slice::<Value>(bytes) {
        Ok(Value::Object(object)) if !asks_remote_code(&object) => Ok(object),
        _ => Err(Cause::RemoteCode),
    }
}

/// Whether any object in the tree has a remote-code member. The parser limits the depth to
/// 128, so this walk is bounded.
fn asks_remote_code(object: &serde_json::Map<String, Value>) -> bool {
    let mut stack = vec![object];
    while let Some(map) = stack.pop() {
        if map
            .keys()
            .any(|key| REMOTE_CODE_MEMBERS.contains(&key.as_str()))
        {
            return true;
        }
        let mut values: Vec<&Value> = map.values().collect();
        while let Some(value) = values.pop() {
            match value {
                Value::Object(inner) => stack.push(inner),
                Value::Array(list) => values.extend(list),
                _ => {}
            }
        }
    }
    false
}

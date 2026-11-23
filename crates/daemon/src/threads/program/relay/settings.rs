//! A setting that can override the child's relay address refuses that start (Section 7.10).

use std::io::Read as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::Path;

pub(super) fn permitted(folder: &Path, env: &[(String, String)]) -> bool {
    // Include ancestors and the user's settings: either can override an environment value.
    let folders: Vec<_> = folder.ancestors().take(129).collect();
    if folders.len() > 128 {
        return false;
    }
    for folder in folders {
        if !settings_clear(&folder.join(".claude")) {
            return false;
        }
    }
    let config = env
        .iter()
        .find(|(name, _)| name == "CLAUDE_CONFIG_DIR")
        .map(|(_, value)| std::path::PathBuf::from(value));
    if let Some(config) = config {
        return config.is_absolute() && settings_clear(&config);
    }
    env.iter()
        .find(|(name, _)| name == "HOME")
        .is_none_or(|(_, home)| settings_clear(&Path::new(home).join(".claude")))
}

fn settings_clear(folder: &Path) -> bool {
    ["settings.json", "settings.local.json"]
        .iter()
        .all(|name| clear(&folder.join(name)))
}

fn clear(path: &Path) -> bool {
    let flags = rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOCTTY;
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(flags.bits().cast_signed())
        .open(path)
    {
        Ok(file) => file,
        Err(error) => return error.kind() == std::io::ErrorKind::NotFound,
    };
    if !file.metadata().is_ok_and(|meta| meta.is_file()) {
        return false;
    }
    let limit = agent_threads::project::MAX_FILE as u64;
    let mut bytes = Vec::new();
    if file.take(limit + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > limit {
        return false;
    }
    let Ok(serde_json::Value::Object(settings)) = serde_json::from_slice(&bytes) else {
        return false;
    };
    match settings.get("env") {
        None => true,
        Some(serde_json::Value::Object(env)) => !env.contains_key("ANTHROPIC_BASE_URL"),
        Some(_) => false,
    }
}

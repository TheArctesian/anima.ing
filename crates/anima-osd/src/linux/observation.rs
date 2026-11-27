//! Owns the boot-local console projection, independently of the diagnostic socket.
mod connections;
mod hardware;

use super::{Error, status};
use nix::unistd::Uid;
use serde::Serialize;
use std::{
    fs,
    os::unix::fs::{MetadataExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

const FRAME_LIMIT: usize = 16_384;
const INTERVAL: Duration = Duration::from_secs(5);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    schema: u8,
    cpu: Option<f64>,
    cpu_collecting: bool,
    memory_total: Option<u64>,
    memory_used: Option<u64>,
    uptime: Option<u64>,
    batteries: Option<Vec<hardware::Battery>>,
    remote_connections: Option<connections::Counts>,
}

fn sample(previous_cpu: &mut Option<hardware::Cpu>) -> Snapshot {
    let current_cpu = status::bounded_text("/proc/stat", 262_144)
        .and_then(|text| hardware::Cpu::from_stat(&text));
    let cpu_collecting = current_cpu.is_some() && previous_cpu.is_none();
    let cpu = current_cpu
        .zip(*previous_cpu)
        .and_then(|(current, previous)| current.usage(previous));
    *previous_cpu = current_cpu;
    let (memory_total, available) = status::memory();
    let memory_used = memory_total
        .zip(available)
        .and_then(|(total, free)| total.checked_sub(free));
    Snapshot {
        schema: 1,
        cpu,
        cpu_collecting,
        memory_total,
        memory_used,
        uptime: status::uptime(),
        batteries: hardware::batteries(Path::new("/sys/class/power_supply")),
        remote_connections: connections::observe(),
    }
}

pub fn start(path: PathBuf) -> Result<(), Error> {
    let parent = path.parent().ok_or(Error::Policy(
        "console snapshot requires a parent directory",
    ))?;
    let metadata = fs::symlink_metadata(parent)?;
    if !path.is_absolute()
        || !metadata.is_dir()
        || metadata.uid() != Uid::effective().as_raw()
        || metadata.mode() & 0o022 != 0
        || fs::canonicalize(parent)? != parent
    {
        return Err(Error::Policy(
            "console snapshot parent must be canonical and writable only by its owner",
        ));
    }
    thread::Builder::new()
        .name("anima-observation".to_owned())
        .spawn(move || observe(&path))?;
    Ok(())
}

fn observe(path: &Path) {
    let mut previous_cpu = None;
    let mut failed = false;
    loop {
        let result = publish(path, &sample(&mut previous_cpu));
        if let Err(ref error) = result
            && !failed
        {
            eprintln!("anima-osd: console snapshot could not be published: {error}");
        }
        failed = result.is_err();
        thread::sleep(INTERVAL);
    }
}

fn publish(path: &Path, snapshot: &Snapshot) -> Result<(), Error> {
    use std::io::Write as _;

    let bytes = serde_json::to_vec(snapshot)?;
    if bytes.len() > FRAME_LIMIT {
        return Err(Error::Policy("console snapshot exceeded its size bound"));
    }
    let parent = path.parent().ok_or(Error::Policy(
        "console snapshot requires a parent directory",
    ))?;
    let mut pending = tempfile::NamedTempFile::new_in(parent)?;
    pending.write_all(&bytes)?;
    pending
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o640))?;
    pending.persist(path).map_err(|error| error.error)?;
    Ok(())
}

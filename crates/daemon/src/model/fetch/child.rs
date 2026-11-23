//! The child fetch process (Section 4): it holds no token, no hub origin and no proxy, and verifies the SHA-256 before it opens the archive.

use std::fs::File;
use std::io::{BufRead as _, BufReader, Read as _, Seek as _, SeekFrom, Stdin, Write as _};
use std::path::Path;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::releases::fetch::{
    Checked, FetchOrder, MAX_ORDER_BYTES, UNPACK_DEADLINE, download_deadline, environment_empty,
    reply,
};

use crate::model::{folders, unpack};

/// The first argument of the program in this mode.
pub const FETCH_MODE: &str = "__model-fetch";

const PARENT_GONE: i32 = 3;

/// Exits 0 with a reply word, or another code at a fault.
pub fn run() -> ! {
    let word = reply(work());
    let mut out = std::io::stdout().lock();
    let written = writeln!(out, "{word}").and_then(|()| out.flush());
    std::process::exit(i32::from(written.is_err()))
}

fn work() -> Result<(), Cause> {
    if !environment_empty(std::env::vars_os().map(|(name, _)| name)) {
        return Err(Cause::FetchFailed);
    }
    let mut input = BufReader::new(std::io::stdin());
    let mut line = String::new();
    input
        .by_ref()
        .take(MAX_ORDER_BYTES)
        .read_line(&mut line)
        .map_err(|_| Cause::RuntimeNotConfigured)?;
    let order: FetchOrder = serde_json::from_str(&line).map_err(|_| Cause::RuntimeNotConfigured)?;
    watch_parent(input, order.parent);
    let checked = order.checked()?;
    let mut archive = tempfile::tempfile_in(&order.archive_folder).map_err(|_| Cause::LimitDisk)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Cause::FetchFailed)?;
    let fetched = super::download(
        &order.address,
        checked.hosts,
        order.size_bytes,
        &mut archive,
    );
    let sha = within(&runtime, download_deadline(order.size_bytes), fetched)
        .ok_or(Cause::SourceFailed)??;
    within_deadline(move || finish(archive, &sha, &order, checked))
}

fn watch_parent(mut input: BufReader<Stdin>, parent: u32) {
    #[cfg(target_os = "linux")]
    {
        use rustix::process::{Signal, getppid, set_parent_process_death_signal};
        // The signal is set before the parent check, so a daemon that ends between the two is still seen.
        // The signal fires when the spawning thread ends, so the daemon spawns this from a long-lived worker.
        if set_parent_process_death_signal(Some(Signal::KILL)).is_err() {
            std::process::exit(PARENT_GONE);
        }
        let same =
            getppid().is_some_and(|pid| pid.as_raw_nonzero().get().cast_unsigned() == parent);
        if !same {
            std::process::exit(PARENT_GONE);
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = parent;
    std::thread::spawn(move || {
        let mut sink = [0u8; 64];
        loop {
            match input.read(&mut sink) {
                Ok(0) => break,
                Err(error) if error.kind() != std::io::ErrorKind::Interrupted => break,
                _ => {}
            }
        }
        std::process::exit(PARENT_GONE);
    });
}

/// The timer is made inside the runtime; one made before `block_on` has no reactor and panics.
fn within<T>(
    runtime: &tokio::runtime::Runtime,
    deadline: std::time::Duration,
    work: impl Future<Output = T>,
) -> Option<T> {
    runtime.block_on(async { tokio::time::timeout(deadline, work).await.ok() })
}

/// Past [`UNPACK_DEADLINE`] the child gives back `archive_rejected`, and its exit ends the thread.
fn within_deadline(
    unpack: impl FnOnce() -> Result<(), Cause> + Send + 'static,
) -> Result<(), Cause> {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || send.send(unpack()));
    receive
        .recv_timeout(UNPACK_DEADLINE)
        .map_err(|_| Cause::ArchiveRejected)?
}

/// # Errors
/// `checksum` when the SHA-256 differs; the causes of [`unpack::unpack`].
pub fn finish(
    mut archive: File,
    sha: &str,
    order: &FetchOrder,
    checked: Checked,
) -> Result<(), Cause> {
    if sha != order.sha256 {
        return Err(Cause::Checksum);
    }
    archive
        .seek(SeekFrom::Start(0))
        .map_err(|_| Cause::LimitDisk)?;
    let into = Path::new(&order.unpack_folder);
    let holds = || folders::free_bytes(into).is_ok_and(|free| free >= order.reserve);
    unpack::unpack(archive, checked.format, into, holds)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::within;

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn work_on_a_thread_with_no_runtime_ends_with_its_value() {
        assert_eq!(
            within(&runtime(), Duration::from_secs(5), async { 7 }),
            Some(7)
        );
    }

    #[test]
    fn work_past_its_deadline_gives_nothing() {
        let slow = async { tokio::time::sleep(Duration::from_secs(60)).await };

        assert_eq!(within(&runtime(), Duration::from_millis(10), slow), None);
    }
}

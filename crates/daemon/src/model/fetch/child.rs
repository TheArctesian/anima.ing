//! The child fetch process (Section 4): the daemon program in the mode [`FETCH_MODE`]. It
//! refuses to work with any environment variable, reads its order as JSON on its standard
//! input, downloads the archive into an unnamed file, verifies the SHA-256 before it opens the
//! archive, unpacks it into the folder that the daemon made, and writes one word on its
//! standard output. It holds no token, no hub origin and no proxy.

use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::Path;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::releases::fetch::{
    Checked, FetchOrder, MAX_ORDER_BYTES, download_deadline, environment_empty, reply,
};

use crate::model::{folders, unpack};

/// The first argument of the program in this mode.
pub const FETCH_MODE: &str = "__model-fetch";

/// Runs the child and exits: code 0 with a reply word, or another code at a fault.
pub fn run() -> ! {
    let word = reply(work());
    let mut out = std::io::stdout().lock();
    let written = writeln!(out, "{word}").and_then(|()| out.flush());
    std::process::exit(i32::from(written.is_err()))
}

fn work() -> Result<(), Cause> {
    if !environment_empty(std::env::vars_os().map(|(name, _)| name)) {
        return Err(Cause::SourceFailed);
    }
    let mut text = String::new();
    std::io::stdin()
        .take(MAX_ORDER_BYTES)
        .read_to_string(&mut text)
        .map_err(|_| Cause::RuntimeNotConfigured)?;
    let order: FetchOrder = serde_json::from_str(&text).map_err(|_| Cause::RuntimeNotConfigured)?;
    let checked = order.checked()?;
    let mut archive = tempfile::tempfile_in(&order.archive_folder).map_err(|_| Cause::LimitDisk)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Cause::SourceFailed)?;
    let fetched = super::download(
        &order.address,
        checked.hosts,
        order.size_bytes,
        &mut archive,
    );
    let sha = runtime
        .block_on(tokio::time::timeout(
            download_deadline(order.size_bytes),
            fetched,
        ))
        .map_err(|_| Cause::SourceFailed)??;
    finish(archive, &sha, &order, checked)
}

/// Verifies the SHA-256 `sha` of a downloaded `archive`, then unpacks it into the folder of
/// `order`. Blocking.
///
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

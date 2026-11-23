//! The pull process (Section 4, Section 7): a child of the daemon with an empty environment and
//! no token, hub origin or proxy. It downloads each file of one copy to `partial/<copy>/`,
//! continues a file from its last complete part, reads each file again from byte 0 for its
//! SHA-256, and only then moves the folder to `copies/<copy>/`.

use std::fs::File;
use std::io::{Seek as _, SeekFrom, Write as _};
use std::os::fd::OwnedFd;
use std::path::Path;
use std::time::Duration;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::files::flat_name;
use daemon_core::model_work::order::{PullFile, PullSource};
use daemon_core::model_work::pull::{
    HOSTS, PART_BYTES, PULL_CAUSES, PullOrder, address_segments, pull_deadline, range_start,
    resume_from,
};
use daemon_core::model_work::releases::fetch::{environment_empty, reply_in};
use reqwest::header::{CONTENT_RANGE, RANGE};
use reqwest::{Client, Url};

use super::partial;
use crate::model::fetch::child::{read_order, watch_parent, within};
use crate::model::{fetch, folders};

/// The first argument of the program in this mode.
pub const PULL_MODE: &str = "__model-pull";

/// A file that the source cut is asked again this many times.
const RETRIES: u32 = 3;

/// Exits 0 with a reply word, or another code at a fault.
pub fn run() -> ! {
    let word = reply_in(work(), &PULL_CAUSES);
    let mut out = std::io::stdout().lock();
    let written = writeln!(out, "{word}").and_then(|()| out.flush());
    std::process::exit(i32::from(written.is_err()))
}

fn work() -> Result<(), Cause> {
    if !environment_empty(std::env::vars_os().map(|(name, _)| name)) {
        return Err(Cause::FetchFailed);
    }
    let (order, input) = read_order::<PullOrder>().ok_or(Cause::MemberUnknown)?;
    watch_parent(input, order.parent);
    order.checked()?;
    let models = Path::new(&order.models);
    let name = order.copy.to_string();
    let below = |relative: &Path| folders::private_below(models, relative);
    let partial_root = below(Path::new("partial")).map_err(|_| Cause::LimitDisk)?;
    let folder = below(&Path::new("partial").join(&name)).map_err(|_| Cause::LimitDisk)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Cause::FetchFailed)?;
    let downloads = download_all(&order, &folder);
    within(&runtime, pull_deadline(order.total_bytes()), downloads)
        .ok_or(Cause::SourceFailed)??;
    let kept: Vec<&str> = order
        .files
        .iter()
        .map(|file| flat_name(&file.name))
        .collect();
    partial::keep_only(&folder, &kept).map_err(|_| Cause::LimitDisk)?;
    for file in &order.files {
        verify(&folder, file)?;
    }
    let copies = below(Path::new("copies")).map_err(|_| Cause::LimitDisk)?;
    let moved = partial::move_folder(&partial_root, &copies, &name);
    let name = std::ffi::CString::new(name).map_err(|_| Cause::LimitDisk)?;
    match moved {
        Ok(()) => Ok(()),
        // An earlier pull of this copy moved its folder already; its files were verified then.
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if !matches!(partial::own_folder(&copies, &name), Ok(true)) {
                return Err(Cause::LimitDisk);
            }
            partial::remove_below(&partial_root, &name, 0).map_err(|_| Cause::LimitDisk)
        }
        Err(_) => Err(Cause::LimitDisk),
    }
}

/// The address of `name` at the revision (Section 7): each part one path segment, so a `/` of
/// a folder stays a literal `/` and each other character is encoded (SF-381).
///
/// # Errors
/// `member_unknown` when the address cannot be made.
pub fn address(source: &PullSource, name: &str) -> Result<Url, Cause> {
    let mut url = Url::parse("https://huggingface.co/").map_err(|_| Cause::MemberUnknown)?;
    url.path_segments_mut()
        .map_err(|()| Cause::MemberUnknown)?
        .clear()
        .extend(address_segments(source, name));
    Ok(url)
}

async fn download_all(order: &PullOrder, folder: &OwnedFd) -> Result<(), Cause> {
    let client = fetch::client(&HOSTS)?;
    let reserve = order.reserve;
    let room = || {
        rustix::fs::fstatvfs(folder)
            .is_ok_and(|stat| stat.f_bavail.saturating_mul(stat.f_frsize) >= reserve)
    };
    for file in &order.files {
        let url = address(&order.source, &file.name)?;
        let mut handle =
            partial::open_partial(folder, flat_name(&file.name)).map_err(|_| Cause::LimitDisk)?;
        let mut tries = 0;
        loop {
            let target = Target {
                file: &mut handle,
                size: file.size_bytes,
                room: &room,
            };
            match download(&client, url.clone(), target).await {
                Err(Cause::SourceFailed) if tries < RETRIES => {
                    tries += 1;
                    tokio::time::sleep(Duration::from_secs(2u64 << tries)).await;
                }
                other => break other?,
            }
        }
    }
    Ok(())
}

/// One partial file, its size, and whether the free disk is above the reserve.
struct Target<'a> {
    file: &'a mut File,
    size: u64,
    room: &'a dyn Fn() -> bool,
}

/// Downloads the rest of one file: from the last complete part with a `Range` request, or from
/// byte 0 when the source answers 200 (PULL-006).
async fn download(client: &Client, url: Url, target: Target<'_>) -> Result<(), Cause> {
    let have = target.file.metadata().map_err(|_| Cause::LimitDisk)?.len();
    let Some(start) = resume_from(have, target.size) else {
        return Ok(());
    };
    let mut request = client.get(url);
    if start > 0 {
        request = request.header(RANGE, format!("bytes={start}-"));
    }
    let mut response = request.send().await.map_err(|_| Cause::SourceFailed)?;
    let range = response.headers().get(CONTENT_RANGE);
    let range = range.and_then(|value| value.to_str().ok());
    let from = range_start(response.status().as_u16(), start, range)?;
    let mut written = Written::at(target, from)?;
    while let Some(chunk) = response.chunk().await.map_err(|_| Cause::SourceFailed)? {
        written.write(&chunk)?;
    }
    written.whole()
}

/// The bytes of one file as they arrive: never past its size, and never below the reserve.
struct Written<'a> {
    target: Target<'a>,
    at: u64,
}

impl<'a> Written<'a> {
    fn at(target: Target<'a>, from: u64) -> Result<Self, Cause> {
        target.file.set_len(from).map_err(|_| Cause::LimitDisk)?;
        target
            .file
            .seek(SeekFrom::Start(from))
            .map_err(|_| Cause::LimitDisk)?;
        Ok(Self { target, at: from })
    }

    /// # Errors
    /// `size` when the chunk passes the size (nothing of it is written); `limit_disk` when the
    /// free disk is below the reserve at a new part, or the write fails.
    fn write(&mut self, chunk: &[u8]) -> Result<(), Cause> {
        let next = self
            .at
            .checked_add(chunk.len() as u64)
            .filter(|next| *next <= self.target.size)
            .ok_or(Cause::Size)?;
        if next / PART_BYTES != self.at / PART_BYTES && !(self.target.room)() {
            return Err(Cause::LimitDisk);
        }
        self.target
            .file
            .write_all(chunk)
            .map_err(|_| Cause::LimitDisk)?;
        self.at = next;
        Ok(())
    }

    /// # Errors
    /// `source_failed` when the source ended before the size: the next try continues it.
    fn whole(self) -> Result<(), Cause> {
        (self.at == self.target.size)
            .then_some(())
            .ok_or(Cause::SourceFailed)
    }
}

/// The size and the SHA-256 of the file as it is on disk now (SF-357); a wrong SHA-256 deletes
/// the file (PULL-005).
fn verify(folder: &OwnedFd, file: &PullFile) -> Result<(), Cause> {
    let name = flat_name(&file.name);
    let (size, sha) = partial::size_and_sha256(folder, name).map_err(|_| Cause::LimitDisk)?;
    if size != file.size_bytes {
        return Err(Cause::Size);
    }
    if sha != file.sha256 {
        let _ = partial::delete_file(folder, name);
        return Err(Cause::Checksum);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use daemon_core::model_work::cause::Cause;
    use daemon_core::model_work::pull::PART_BYTES;

    use super::{Target, Written};

    #[test]
    fn pull_005_a_file_stops_at_its_size_and_a_cut_is_not_whole() {
        let mut file = tempfile::tempfile().expect("a file");
        let room = || true;
        let target = Target {
            file: &mut file,
            size: 4,
            room: &room,
        };
        let mut written = Written::at(target, 0).expect("start");

        written.write(b"abc").expect("within");
        assert_eq!(written.write(b"de"), Err(Cause::Size));
        assert_eq!(written.whole(), Err(Cause::SourceFailed));
        assert_eq!(file.metadata().expect("meta").len(), 3);
    }

    #[test]
    fn pull_limit_disk_a_new_part_below_the_reserve_stops() {
        let mut file = tempfile::tempfile().expect("a file");
        let room = || false;
        let target = Target {
            file: &mut file,
            size: PART_BYTES + 1,
            room: &room,
        };
        let mut written = Written::at(target, PART_BYTES - 1).expect("start");

        assert_eq!(written.write(b"xy"), Err(Cause::LimitDisk));
    }
}

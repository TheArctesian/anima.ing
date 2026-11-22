//! The files that the person attaches to a prompt (TH111). Each part is written to disk as it
//! comes, below `attachments/<thread>/pending/<id>/` in the private folder of the threads; a
//! file that goes with a prompt moves to `attachments/<thread>/<id>/`, where the program reads
//! it. Nothing of a file is kept in memory but its facts, and nothing of it goes to a log.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_threads::control::Refusal;
use agent_threads::control::attach::{
    self, ImageKind, MOST_ATTACHMENTS, MOST_IMAGE_BYTES_IN_PROMPT, MOST_PART_BYTES,
};
use agent_threads::drive::{Attached, Attachment, Program};
use base64::Engine as _;
use uuid::Uuid;

/// How long a file that did not go with a prompt is kept.
pub const PENDING_FOR: Duration = Duration::from_secs(60 * 60);
/// The most files that wait to go, of all threads.
const MOST_PENDING: usize = 200;
/// The most bytes of the files that wait to go, of all threads.
const MOST_PENDING_BYTES: u64 = 1024 * 1024 * 1024;
/// The most bytes of one image or PDF that goes inside the line of Claude Code: its limit is about
/// 5 MB in base64.
const MOST_INLINE_BYTES: u64 = 3_932_160;
/// The most bytes of the files inside one line of Claude Code, so that the line fits in a frame
/// to the holder with room.
const MOST_INLINE_BYTES_IN_PROMPT: u64 = 16 * 1024 * 1024;

/// A file that is coming or waits to go.
#[derive(Debug)]
struct Upload {
    thread: Uuid,
    name: String,
    image: Option<ImageKind>,
    size: u64,
    received: u64,
    first: Vec<u8>,
    since: Instant,
}

/// The files of the threads.
#[derive(Debug)]
pub struct Attachments {
    folder: PathBuf,
    pending: BTreeMap<Uuid, Upload>,
}

impl Attachments {
    /// The files below `threads/attachments`. A file that did not go before the runner started
    /// is removed: nothing knows of it any more.
    #[must_use]
    pub fn at(threads: &Path) -> Self {
        let folder = threads.join("attachments");
        if let Ok(entries) = std::fs::read_dir(&folder) {
            for entry in entries.flatten() {
                remove(&entry.path().join("pending"));
            }
        }
        Self {
            folder,
            pending: BTreeMap::new(),
        }
    }

    /// A new file and its first part.
    ///
    /// # Errors
    /// [`Refusal::TooLarge`] over a limit, [`Refusal::CannotAttach`] when the file cannot be
    /// written, and [`Refusal::BadValue`] for a part that is not base64.
    pub fn begin(
        &mut self,
        thread: Uuid,
        name: &str,
        mime: &str,
        size: u64,
    ) -> Result<Uuid, Refusal> {
        let name = attach::safe_name(name);
        let most = attach::most_bytes(mime);
        if size == 0 {
            return Err(cannot(&name, "it is empty"));
        }
        if size > most {
            return Err(too_large(
                &name,
                &format!("the limit is {} MiB", most >> 20),
            ));
        }
        let waiting: u64 = self.pending.values().map(|upload| upload.size).sum();
        if self.pending.len() >= MOST_PENDING || waiting + size > MOST_PENDING_BYTES {
            return Err(too_large(
                &name,
                "too many files wait to go; send or drop some first",
            ));
        }
        let id = Uuid::now_v7();
        let folder = self.pending_folder(thread, id);
        make_private(&folder).map_err(|_| cannot(&name, "the runner could not write it"))?;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(folder.join(&name))
            .map_err(|_| cannot(&name, "the runner could not write it"))?;
        let upload = Upload {
            thread,
            name,
            image: ImageKind::of_mime(mime),
            size,
            received: 0,
            first: Vec::new(),
            since: Instant::now(),
        };
        self.pending.insert(id, upload);
        Ok(id)
    }

    /// The next part of a file: the bytes received so far.
    ///
    /// # Errors
    /// [`Refusal::CannotAttach`] for a file that is not of the thread or is complete, or whose
    /// bytes are not the image its type says, [`Refusal::TooLarge`] for a part over
    /// [`MOST_PART_BYTES`] or past the size, and [`Refusal::BadValue`] for a part that is not base64.
    pub fn add(&mut self, thread: Uuid, id: Uuid, base64: &str) -> Result<u64, Refusal> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(base64)
            .map_err(|_| Refusal::BadValue)?;
        let path = self.pending_path(thread, id)?;
        let upload = self.pending.get_mut(&id).ok_or_else(gone)?;
        let after = upload
            .received
            .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        if bytes.len() > MOST_PART_BYTES || after > upload.size {
            let name = upload.name.clone();
            self.drop_pending(id);
            return Err(too_large(
                &name,
                "a part is larger than the file or than 2 MiB",
            ));
        }
        let written = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .and_then(|mut file| file.write_all(&bytes));
        if written.is_err() {
            let name = upload.name.clone();
            self.drop_pending(id);
            return Err(cannot(&name, "the runner could not write it"));
        }
        upload.received = after;
        let room = 16usize.saturating_sub(upload.first.len());
        upload.first.extend(bytes.iter().take(room));
        if upload.received == upload.size && !upload.matches() {
            let name = upload.name.clone();
            self.drop_pending(id);
            return Err(cannot(
                &name,
                "its bytes are not the image that its type says",
            ));
        }
        Ok(after)
    }

    /// Drops a file that did not go.
    ///
    /// # Errors
    /// [`Refusal::CannotAttach`] when no such file of the thread waits.
    pub fn drop_file(&mut self, thread: Uuid, id: Uuid) -> Result<(), Refusal> {
        self.pending_path(thread, id)?;
        self.drop_pending(id);
        Ok(())
    }

    /// The files of a prompt, moved to where the program reads them. A file whose bytes go inside
    /// the line of Claude Code is read here. A refusal moves nothing.
    ///
    /// # Errors
    /// [`Refusal::CannotAttach`] for a file that is not of the thread or not complete, and
    /// [`Refusal::TooLarge`] over the count or the bytes of images of one prompt.
    pub fn take(
        &mut self,
        thread: Uuid,
        ids: &[Uuid],
        program: Program,
    ) -> Result<Vec<Attachment>, Refusal> {
        self.check(thread, ids)?;
        let mut inline_room = MOST_INLINE_BYTES_IN_PROMPT;
        let mut taken = Vec::new();
        for id in ids {
            let Some(upload) = self.pending.remove(id) else {
                continue;
            };
            let from = self.pending_folder(thread, *id);
            let to = self.folder.join(thread.to_string()).join(id.to_string());
            let moved = to
                .parent()
                .map_or(Ok(()), make_private)
                .and_then(|()| std::fs::rename(&from, &to));
            if moved.is_err() {
                remove(&from);
                return Err(cannot(&upload.name, "the runner could not move it"));
            }
            let path = to.join(&upload.name);
            let inline = (program == Program::ClaudeCode
                && upload.size <= MOST_INLINE_BYTES
                && upload.size <= inline_room
                && (upload.image.is_some() || attach::is_pdf(&upload.first)))
            .then(|| std::fs::read(&path).ok())
            .flatten()
            .map(|bytes| {
                inline_room -= upload.size;
                base64::engine::general_purpose::STANDARD.encode(bytes)
            });
            let kind = match upload.image {
                Some(kind) => Attached::Image { kind, inline },
                None if attach::is_pdf(&upload.first) => Attached::Pdf { inline },
                None => Attached::File,
            };
            taken.push(Attachment {
                name: upload.name,
                path,
                kind,
            });
        }
        Ok(taken)
    }

    /// Removes the files that waited longer than [`PENDING_FOR`].
    pub fn expire(&mut self, now: Instant) {
        let old: Vec<Uuid> = self
            .pending
            .iter()
            .filter(|(_, upload)| now.duration_since(upload.since) > PENDING_FOR)
            .map(|(id, _)| *id)
            .collect();
        for id in old {
            self.drop_pending(id);
        }
    }

    /// Removes each file of a thread that the person removed.
    pub fn forget(&mut self, thread: Uuid) {
        self.pending.retain(|_, upload| upload.thread != thread);
        remove(&self.folder.join(thread.to_string()));
    }

    fn check(&self, thread: Uuid, ids: &[Uuid]) -> Result<(), Refusal> {
        if ids.len() > MOST_ATTACHMENTS {
            return Err(too_large(
                "the prompt",
                &format!("at most {MOST_ATTACHMENTS} files go with one prompt"),
            ));
        }
        let mut images = 0;
        for (at, id) in ids.iter().enumerate() {
            let upload = self
                .pending
                .get(id)
                .filter(|upload| upload.thread == thread);
            let upload = upload.ok_or_else(gone)?;
            if upload.received != upload.size {
                return Err(cannot(&upload.name, "not all of it came yet"));
            }
            if ids[..at].contains(id) {
                return Err(cannot(&upload.name, "it is named twice"));
            }
            if upload.image.is_some() {
                images += upload.size;
            }
        }
        if images > MOST_IMAGE_BYTES_IN_PROMPT {
            return Err(too_large(
                "the images",
                "at most 80 MiB of images go with one prompt",
            ));
        }
        Ok(())
    }

    fn pending_folder(&self, thread: Uuid, id: Uuid) -> PathBuf {
        self.folder
            .join(thread.to_string())
            .join("pending")
            .join(id.to_string())
    }

    fn pending_path(&self, thread: Uuid, id: Uuid) -> Result<PathBuf, Refusal> {
        let upload = self
            .pending
            .get(&id)
            .filter(|upload| upload.thread == thread);
        let upload = upload.ok_or_else(gone)?;
        Ok(self.pending_folder(thread, id).join(&upload.name))
    }

    fn drop_pending(&mut self, id: Uuid) {
        if let Some(upload) = self.pending.remove(&id) {
            remove(&self.pending_folder(upload.thread, id));
        }
    }
}

impl Upload {
    /// A file that says it is an image starts with the bytes of that kind of image.
    fn matches(&self) -> bool {
        self.image
            .is_none_or(|kind| ImageKind::of_bytes(&self.first) == Some(kind))
    }
}

/// Makes a folder, and each folder above it that is missing, for this user only.
fn make_private(folder: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt as _;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(folder)
}

fn remove(path: &Path) {
    if let Err(error) = std::fs::remove_dir_all(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(%error, "a folder of attachments was not removed");
    }
}

fn gone() -> Refusal {
    Refusal::CannotAttach {
        said: "That file is not attached to this thread, or it went already. Attach it again."
            .to_owned(),
    }
}

fn cannot(name: &str, why: &str) -> Refusal {
    Refusal::CannotAttach {
        said: format!("{name} cannot go with the prompt: {why}."),
    }
}

fn too_large(name: &str, why: &str) -> Refusal {
    Refusal::TooLarge {
        said: format!("{name} is too large: {why}."),
    }
}

#[cfg(test)]
mod tests;

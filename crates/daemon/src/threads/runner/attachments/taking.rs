//! The files of a prompt on their way to the program (TH111): they move to where the program
//! reads them before the line goes, and back to the files that wait when the line does not go,
//! so that a failed prompt never loses a file of the person.

use std::path::Path;
use std::time::Instant;

use agent_threads::control::Refusal;
use agent_threads::control::attach::{self, ImageKind};
use agent_threads::drive::{Attached, Attachment, Program};
use base64::Engine as _;
use uuid::Uuid;

use super::{Attachments, Upload, cannot, make_private, remove};

/// The most bytes of one image that goes inside the line of Claude Code: its limit is about 5 MB
/// in base64.
const MOST_INLINE_BYTES: u64 = 3_932_160;
/// The most bytes of the images inside one line of Claude Code. The images stay in the history
/// of the conversation, and each later request carries them again: a few per prompt keep the
/// requests of a long thread under the limit of the API.
const MOST_INLINE_BYTES_IN_PROMPT: u64 = 5 * 1024 * 1024;
/// The most images inside one line of Claude Code; the others go by their path.
const MOST_INLINE_IMAGES: u32 = 5;
/// The most pixels on a side of an image that goes inside the line of Claude Code: the API takes
/// no larger side in a request with many images.
const MOST_INLINE_SIDE: u32 = 2000;

/// What is left for images inside one line.
#[derive(Debug)]
struct Room {
    bytes: u64,
    images: u32,
}

impl Attachments {
    /// The files of a prompt, moved to where the program reads them; each waits for
    /// [`Self::settle`] or [`Self::restore`]. An image that goes inside the line of Claude Code
    /// is read here. A refusal moves nothing.
    ///
    /// # Errors
    /// [`Refusal::CannotAttach`] for a file that is not of the thread, not complete, or that
    /// cannot move, and [`Refusal::TooLarge`] over the count or the bytes of images of one prompt.
    pub fn take(
        &mut self,
        thread: Uuid,
        ids: &[Uuid],
        program: Program,
    ) -> Result<Vec<Attachment>, Refusal> {
        self.check(thread, ids)?;
        for (at, id) in ids.iter().enumerate() {
            if let Err(why) = self.move_out(thread, *id) {
                self.restore(thread, &ids[..at]);
                return Err(why);
            }
        }
        let mut room = Room {
            bytes: MOST_INLINE_BYTES_IN_PROMPT,
            images: MOST_INLINE_IMAGES,
        };
        let taken = ids
            .iter()
            .filter_map(|id| self.going.get(id).map(|upload| (*id, upload)))
            .map(|(id, upload)| {
                let path = self
                    .folder
                    .join(thread.to_string())
                    .join(id.to_string())
                    .join(&upload.name);
                attachment(upload, &path, program == Program::ClaudeCode, &mut room)
            })
            .collect();
        Ok(taken)
    }

    /// The line of the prompt went: its files stay where the program reads them.
    pub fn settle(&mut self, ids: &[Uuid]) {
        for id in ids {
            self.going.remove(id);
        }
    }

    /// The line of the prompt did not go: its files wait again, for a new hour.
    pub fn restore(&mut self, thread: Uuid, ids: &[Uuid]) {
        for id in ids {
            let Some(mut upload) = self
                .going
                .remove(id)
                .filter(|upload| upload.thread == thread)
            else {
                continue;
            };
            let from = self.folder.join(thread.to_string()).join(id.to_string());
            let to = self.pending_folder(thread, *id);
            let back = to
                .parent()
                .map_or(Ok(()), make_private)
                .and_then(|()| std::fs::rename(&from, &to));
            if back.is_err() {
                remove(&from);
                continue;
            }
            upload.since = Instant::now();
            self.pending.insert(*id, upload);
        }
    }

    fn move_out(&mut self, thread: Uuid, id: Uuid) -> Result<(), Refusal> {
        let Some(upload) = self.pending.remove(&id) else {
            return Ok(());
        };
        let from = self.pending_folder(thread, id);
        let to = self.folder.join(thread.to_string()).join(id.to_string());
        let moved = to
            .parent()
            .map_or(Ok(()), make_private)
            .and_then(|()| std::fs::rename(&from, &to));
        if moved.is_err() {
            let why = cannot(&upload.name, "the runner could not move it");
            self.pending.insert(id, upload);
            return Err(why);
        }
        self.going.insert(id, upload);
        Ok(())
    }
}

/// A file as the program gets it. An image goes inside the line of Claude Code when it is small,
/// its header reads, and its sides fit; each other file goes by its path, which the program reads
/// with its own tools (they make a large image smaller and read a PDF page by page).
fn attachment(upload: &Upload, path: &Path, claude: bool, room: &mut Room) -> Attachment {
    let kind = match upload.image {
        Some(kind) => {
            let small = claude
                && room.images > 0
                && upload.size <= MOST_INLINE_BYTES
                && upload.size <= room.bytes;
            let inline = small
                .then(|| inline_image(kind, path, upload.size))
                .flatten();
            if inline.is_some() {
                room.bytes -= upload.size;
                room.images -= 1;
            }
            Attached::Image { kind, inline }
        }
        None if attach::is_pdf(&upload.first) => Attached::Pdf { inline: None },
        None => Attached::File,
    };
    Attachment {
        name: upload.name.clone(),
        path: path.to_owned(),
        kind,
    }
}

/// The bytes of an image in base64, when they are the `size` that came, its header reads and its
/// sides fit.
fn inline_image(kind: ImageKind, path: &Path, size: u64) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if u64::try_from(bytes.len()).ok()? != size {
        return None;
    }
    let (width, height) = kind.size(&bytes)?;
    let fits = (1..=MOST_INLINE_SIDE).contains(&width) && (1..=MOST_INLINE_SIDE).contains(&height);
    fits.then(|| base64::engine::general_purpose::STANDARD.encode(bytes))
}

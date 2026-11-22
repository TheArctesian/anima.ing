//! `anima agents send`: a prompt to a thread that this computer runs, with files (TH111). Each
//! file goes to the runner in parts, then the prompt names them.

use std::path::{Path, PathBuf};

use agent_threads::control::attach::{
    self, ImageKind, MOST_ATTACHMENTS, MOST_FILE_BYTES, MOST_PART_BYTES,
};
use agent_threads::control::{Action, Outcome};
use base64::Engine as _;
use uuid::Uuid;

use super::super::runner::RunnerLink;
use super::{accepted, simple};
use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(Debug, clap::Args)]
pub struct SendArgs {
    thread: Uuid,
    /// The prompt: to the running turn, or as a new turn.
    text: String,
    /// A file to send with the prompt; give it again for each file. An image (png, jpeg, gif,
    /// webp) is at most 10 MiB, a different file at most 50 MiB.
    #[arg(long = "attach", value_name = "FILE")]
    attach: Vec<PathBuf>,
}

impl SendArgs {
    /// # Errors
    /// [`Failure`] for a file that cannot be read and for a refusal of the runner.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        if self.attach.is_empty() {
            let action = Action::Send {
                thread: self.thread,
                text: self.text,
                attachments: Vec::new(),
            };
            return simple(invocation, action, "Sent.").await;
        }
        if self.attach.len() > MOST_ATTACHMENTS {
            return Err(Failure::Usage {
                message: format!("At most {MOST_ATTACHMENTS} files go with one prompt."),
                hint: None,
            });
        }
        let mut link = RunnerLink::connect().await?;
        let mut attachments = Vec::new();
        for path in &self.attach {
            match attach_file(&mut link, self.thread, path).await {
                Ok(id) => attachments.push(id),
                Err(failure) => {
                    unattach(&mut link, self.thread, attachments).await;
                    return Err(failure);
                }
            }
        }
        let send = Action::Send {
            thread: self.thread,
            text: self.text,
            attachments: attachments.clone(),
        };
        if let Err(failure) = link.call(send).await.and_then(accepted) {
            unattach(&mut link, self.thread, attachments).await;
            return Err(failure);
        }
        invocation.output.message("Sent.");
        Ok(())
    }
}

/// Drops the files that wait for a prompt that did not go: now, not after an hour.
async fn unattach(link: &mut RunnerLink, thread: Uuid, attachments: Vec<Uuid>) {
    for attachment in attachments {
        let _ = link.call(Action::Unattach { thread, attachment }).await;
    }
}

/// Sends one file in parts; gives its ID.
async fn attach_file(link: &mut RunnerLink, thread: Uuid, path: &Path) -> Result<Uuid, Failure> {
    let unreadable = |reason: String| Failure::Input {
        what: path.display().to_string(),
        reason,
    };
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let size = std::fs::metadata(path)
        .map_err(|error| unreadable(error.to_string()))?
        .len();
    if size > MOST_FILE_BYTES {
        return Err(unreadable(format!(
            "it is larger than {} MiB",
            MOST_FILE_BYTES >> 20
        )));
    }
    let bytes = std::fs::read(path).map_err(|error| unreadable(error.to_string()))?;
    let mime = mime_of(path, &bytes);
    if size > attach::most_bytes(mime) {
        return Err(unreadable(format!(
            "it is larger than {} MiB",
            attach::most_bytes(mime) >> 20
        )));
    }
    let mut parts = bytes.chunks(MOST_PART_BYTES);
    let first = Action::Attach {
        thread,
        name,
        mime: mime.to_owned(),
        size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        bytes: encoded(parts.next().unwrap_or_default()),
        draft: false,
    };
    let Outcome::Attached { id, .. } = accepted(link.call(first).await?)? else {
        return Err(unreadable("the runner did not take it".to_owned()));
    };
    for part in parts {
        let more = Action::AttachMore {
            thread,
            attachment: id,
            bytes: encoded(part),
        };
        accepted(link.call(more).await?)?;
    }
    Ok(id)
}

fn encoded(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// The MIME type of a file: an image by its first bytes, as the runner checks it, else by its
/// name.
fn mime_of(path: &Path, bytes: &[u8]) -> &'static str {
    if let Some(kind) = ImageKind::of_bytes(bytes) {
        return kind.mime();
    }
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase());
    match extension.as_deref() {
        Some("pdf") => "application/pdf",
        Some("txt" | "md" | "log") => "text/plain",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn th111_the_type_of_an_image_comes_from_its_bytes_and_of_a_file_from_its_name() {
        assert_eq!(
            super::mime_of(Path::new("a/shot.png"), b"\xff\xd8\xff\xe0"),
            "image/jpeg"
        );
        assert_eq!(
            super::mime_of(Path::new("b.pdf"), b"%PDF-1.4"),
            "application/pdf"
        );
        assert_eq!(
            super::mime_of(Path::new("c.png"), b"not a png"),
            "application/octet-stream"
        );
    }
}

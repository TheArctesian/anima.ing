//! A prompt of the person with the files that go with it (TH111), and the words that name the
//! files that a program reads from disk. The runner wrote each file before; it reads the bytes
//! of a file that goes inside the line.

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::control::attach::ImageKind;

/// A file that goes with a prompt: the runner wrote it at `path`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    /// The name, made safe ([`crate::control::attach::safe_name`]).
    pub name: String,
    pub path: PathBuf,
    pub kind: Attached,
}

/// What a file is, and its bytes in base64 when they go inside the line of a program that takes
/// them there (Claude Code: small images, measured on 2.1.311, 2026-11-21).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attached {
    Image {
        kind: ImageKind,
        inline: Option<String>,
    },
    Pdf {
        inline: Option<String>,
    },
    File,
}

impl Attached {
    /// The bytes in base64 that go inside the line, if any.
    #[must_use]
    pub fn inline(&self) -> Option<&str> {
        match self {
            Self::Image { inline, .. } | Self::Pdf { inline } => inline.as_deref(),
            Self::File => None,
        }
    }
}

/// The words of a prompt and its files.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Prompt {
    pub text: String,
    pub attachments: Vec<Attachment>,
}

impl Prompt {
    /// A prompt with no file.
    #[must_use]
    pub fn plain(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            attachments: Vec::new(),
        }
    }

    /// How many images go with the prompt.
    #[must_use]
    pub fn images(&self) -> u32 {
        let images = self
            .attachments
            .iter()
            .filter(|file| matches!(file.kind, Attached::Image { .. }))
            .count();
        u32::try_from(images).unwrap_or(u32::MAX)
    }

    /// The names of the files that are not images.
    #[must_use]
    pub fn files(&self) -> Vec<String> {
        self.attachments
            .iter()
            .filter(|file| !matches!(file.kind, Attached::Image { .. }))
            .map(|file| file.name.clone())
            .collect()
    }

    /// The words, with the path of each file that `by_path` says the program reads from disk. Only
    /// the paths are written, one on each line: the name of a file is the last part of its path,
    /// so a name cannot point at a different file.
    #[must_use]
    pub fn words(&self, by_path: impl Fn(&Attachment) -> bool) -> String {
        let mut words = self.text.clone();
        let mut named = self
            .attachments
            .iter()
            .filter(|file| by_path(file))
            .peekable();
        if named.peek().is_some() {
            words.push_str("\n\nThe person attached these files. Read each one at its path:");
            for file in named {
                let _ = write!(words, "\n- {}", file.path.display());
            }
        }
        words
    }
}

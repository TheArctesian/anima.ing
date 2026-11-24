//! A prompt of the person with the files that go with it (TH111), and the words that name the
//! files that a program reads from disk. The runner wrote each file before; it reads the bytes
//! of a file that goes inside the line.

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::control::attach::ImageKind;

/// A file that goes with a prompt: the runner wrote it at `path`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    /// The name as the person sees it ([`crate::control::attach::shown_name`]); never a part of
    /// [`Self::path`].
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

/// The words of a prompt, its files and the skills that the person chose for it (TH117).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Prompt {
    pub text: String,
    pub attachments: Vec<Attachment>,
    /// Codex only: each is a skill that the program listed for the thread.
    pub skills: Vec<ChosenSkill>,
}

/// A skill that a prompt names, as the program listed it (TH117).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChosenSkill {
    pub name: String,
    pub path: String,
}

impl Prompt {
    /// A prompt with no file.
    #[must_use]
    pub fn plain(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            ..Self::default()
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

    /// The words, with the path of each file that `by_path` says the program reads from disk, one
    /// on each line. The path holds no text of the person; the name that the person gave follows
    /// it as a JSON string, only to say what the file is.
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
                let name = serde_json::to_string(&file.name).unwrap_or_default();
                let _ = write!(words, "\n- {} (named {name})", file.path.display());
            }
        }
        words
    }
}

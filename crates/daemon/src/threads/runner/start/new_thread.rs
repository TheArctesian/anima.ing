//! A new thread as a start call names it.

use std::path::PathBuf;

use agent_threads::drive::{Effort, Mode, Program};
use uuid::Uuid;

/// A new thread as the client named it.
pub(in crate::threads::runner) struct NewThread {
    pub program: Program,
    pub folder: PathBuf,
    pub mode: Mode,
    pub model: Option<String>,
    pub effort: Option<Effort>,
    pub prompt: String,
    pub program_path: Option<PathBuf>,
    /// The draft whose files go with the first prompt (TH111).
    pub draft: Option<Uuid>,
    pub attachments: Vec<Uuid>,
}

impl NewThread {
    /// The new thread of a start call; `None` for a different call, and for a start that asks
    /// Codex to plan first, which Codex cannot do (TH105).
    pub(in crate::threads::runner) fn of(action: agent_threads::control::Action) -> Option<Self> {
        match action {
            agent_threads::control::Action::Start {
                program,
                folder,
                mode,
                model,
                effort,
                prompt,
                program_path,
                draft,
                attachments,
            } if !(program == Program::Codex && mode == Mode::Plan) => Some(Self {
                program,
                folder,
                mode,
                model,
                effort,
                prompt,
                program_path,
                draft,
                attachments,
            }),
            _ => None,
        }
    }
}

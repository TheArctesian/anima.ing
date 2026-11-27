//! Recreate Codex settings from the runner's persisted thread row.
use agent_threads::{
    control::ThreadRow,
    drive::{Effort, KeptFolder, Model, Program},
};

use super::Codex;

impl Codex {
    /// The Codex calls of a thread of Codex.
    /// Its sandbox keeps the folder of the daemon read-only when the folder of the thread holds it
    /// (THR-041).
    pub(in crate::threads::runner) fn for_row(
        row: &ThreadRow,
        effort: Option<Effort>,
        daemon: &std::path::Path,
    ) -> Option<Self> {
        (row.program == Program::Codex).then(|| {
            let model = row.model.as_deref().and_then(|name| Model::new(name).ok());
            let mut codex = Self::new(row.folder.clone(), row.mode, model, effort).keeping(
                crate::threads::notice::held_anima(&row.folder, daemon)
                    .map(|anima| KeptFolder::new(anima, row.id)),
            );
            codex.change_speed(row.speed.clone());
            codex
        })
    }
}

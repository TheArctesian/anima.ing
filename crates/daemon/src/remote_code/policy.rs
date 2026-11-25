//! TH33 local settings and the same sharing eligibility check used by the K8 publisher.

use std::path::{Path, PathBuf};

use agent_threads::drive::{Mode, Program};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::wire::LaunchParameters;
use crate::{
    config::{agent_usage::UsageSettings, home::Home, private_file},
    readers::agent_sessions::shared_repository,
};

const SETTINGS: &str = "remote-code-settings.json";

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct Settings {
    threads: bool,
    allowed_folders: Vec<PathBuf>,
    ceiling: Mode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    ThreadsOff,
    ProgramNotPermitted,
    FolderNotPermitted,
    TargetUnavailable,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct Target {
    pub project: Uuid,
    pub folder: PathBuf,
    pub program: Program,
}

/// Current owner configuration. Open a fresh snapshot at each authorization boundary.
pub struct Policy {
    settings: Settings,
    sharing: Option<UsageSettings>,
    roots: Vec<PathBuf>,
    user: PathBuf,
}

impl Policy {
    #[must_use]
    pub fn read(home: &Home, user: PathBuf) -> Self {
        let settings = read_settings(home).unwrap_or(Settings {
            ceiling: Mode::Plan,
            ..Settings::default()
        });
        let sharing = crate::runtime::thread_sync::transcripts_enabled(home)
            .then(|| UsageSettings::read(home).ok())
            .flatten()
            .filter(|settings| !settings.off);
        let roots = settings
            .allowed_folders
            .iter()
            .filter(|path| real(path).as_ref() == Some(path))
            .cloned()
            .collect();
        Self {
            settings,
            sharing,
            roots,
            user,
        }
    }

    #[must_use]
    pub const fn starts_enabled(&self) -> bool {
        self.settings.threads
    }

    #[must_use]
    pub const fn ceiling(&self) -> Mode {
        self.settings.ceiling
    }

    #[must_use]
    pub fn publishable(&self, folder: &Path) -> bool {
        self.sharing
            .as_ref()
            .is_some_and(|settings| shared_repository(&self.user, folder, settings).is_some())
    }

    /// Return only complete pairs from the inventory passed by the local publisher.
    #[must_use]
    pub fn targets(&self, candidates: impl IntoIterator<Item = Target>) -> Vec<Target> {
        if !self.starts_enabled() {
            return Vec::new();
        }
        candidates
            .into_iter()
            .filter_map(|mut target| {
                target.folder = real(&target.folder)?;
                target.folder.to_str()?;
                (!target.project.is_nil() && self.eligible(&target.folder, target.program).is_ok())
                    .then_some(target)
            })
            .collect()
    }

    /// Recheck the exact offered project, canonical folder and program.
    ///
    /// # Errors
    /// The fixed local refusal explaining why the pair cannot start.
    pub fn check_launch(
        &self,
        project: Uuid,
        launch: &LaunchParameters,
        targets: &[Target],
    ) -> Result<Mode, Refusal> {
        self.eligible(&launch.folder, launch.program)?;
        if !targets.iter().any(|target| {
            target.project == project
                && target.folder == launch.folder
                && target.program == launch.program
        }) {
            return Err(Refusal::TargetUnavailable);
        }
        let mode = lower(launch.access, self.ceiling());
        if mode == Mode::Plan && launch.program != Program::ClaudeCode {
            return Err(Refusal::ProgramNotPermitted);
        }
        Ok(mode)
    }

    #[must_use]
    pub fn permitted_folder(&self, folder: &Path) -> bool {
        real(folder).as_deref() == Some(folder)
            && self.roots.iter().any(|root| folder.starts_with(root))
    }

    fn eligible(&self, folder: &Path, program: Program) -> Result<(), Refusal> {
        if !self.starts_enabled() {
            return Err(Refusal::ThreadsOff);
        }
        if !matches!(program, Program::ClaudeCode | Program::Codex) {
            return Err(Refusal::ProgramNotPermitted);
        }
        if !self.permitted_folder(folder) {
            return Err(Refusal::FolderNotPermitted);
        }
        if !self.publishable(folder) {
            return Err(Refusal::TargetUnavailable);
        }
        Ok(())
    }
}

fn read_settings(home: &Home) -> Option<Settings> {
    match private_file::read(&home.dir().join(SETTINGS)).ok()? {
        Some(bytes) => serde_json::from_slice(&bytes).ok(),
        None => Some(Settings::default()),
    }
}

fn real(folder: &Path) -> Option<PathBuf> {
    if !folder.is_absolute() {
        return None;
    }
    let real = folder.canonicalize().ok()?;
    real.is_dir().then_some(real)
}

pub(super) fn lower(left: Mode, right: Mode) -> Mode {
    let rank = |mode| match mode {
        Mode::Plan => 0,
        Mode::Ask => 1,
        Mode::AcceptEdits => 2,
        Mode::FullAccess => 3,
    };
    if rank(left) <= rank(right) {
        left
    } else {
        right
    }
}

#[cfg(test)]
mod tests;

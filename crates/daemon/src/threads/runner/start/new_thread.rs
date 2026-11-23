//! A new thread as a start call names it.

use std::path::{Path, PathBuf};

use agent_threads::control::{RunState, ThreadRow, Workspace};
use agent_threads::drive::{Effort, Launch, Mode, Model, Program, SessionStart};
use uuid::Uuid;

use super::super::accounts::STANDARD;
use super::super::{Runner, login};
use super::chosen_folder;
use crate::threads::wire;
use agent_threads::control::Refusal;

/// A new thread as the client named it.
pub(in crate::threads::runner) struct NewThread {
    pub program: Program,
    pub folder: PathBuf,
    pub mode: Mode,
    pub model: Option<String>,
    pub effort: Option<Effort>,
    /// A speed of the model (TH117).
    pub speed: Option<String>,
    pub prompt: String,
    pub program_path: Option<PathBuf>,
    /// The draft whose files go with the first prompt (TH111).
    pub draft: Option<Uuid>,
    pub attachments: Vec<Uuid>,
    /// "New worktree": the thread runs in a worktree of its own of the repository at `folder`
    /// (TC7, TH116).
    pub worktree: Option<NewWorktree>,
}

/// The worktree of a new thread.
#[derive(Clone, Debug)]
pub(in crate::threads::runner) struct NewWorktree {
    /// The branch to start from; `HEAD` of the project when `None`.
    pub base: Option<String>,
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
                speed,
                prompt,
                program_path,
                draft,
                attachments,
                worktree,
                base,
            } if !(program == Program::Codex && mode == Mode::Plan) => Some(Self {
                program,
                folder,
                mode,
                model,
                effort,
                speed,
                prompt,
                program_path,
                draft,
                attachments,
                worktree: worktree.then_some(NewWorktree { base }),
            }),
            _ => None,
        }
    }
}

impl Runner {
    /// The row of a new thread and the start for the holder: the folder that the person chose
    /// (THR-041), the program on the `PATH` of the login, the arguments of the closed list
    /// (THR-040) and the environment of the login (R-308).
    pub(super) fn prepared(
        &self,
        new: &NewThread,
        id: Uuid,
        path: &Path,
    ) -> Result<(ThreadRow, wire::Action), Refusal> {
        let folder = chosen_folder(&new.folder, self.dir.parent().unwrap_or(&self.dir))?;
        let model = new
            .model
            .as_deref()
            .map(Model::new)
            .transpose()
            .map_err(|_| Refusal::BadValue)?;
        let launch = Launch {
            program: new.program,
            path: path.to_owned(),
            mode: new.mode,
            model: model.clone(),
            effort: new.effort,
            session: SessionStart::New(id),
        };
        let invocation = launch.invocation().map_err(|_| Refusal::BadValue)?;
        // The `TMPDIR` of the agent is made before the thread has a seat (THR-050).
        let tmp = super::private_tmp::made(&self.dir, id).map_err(|error| {
            tracing::warn!(%error, %id, "the temporary folder of a thread could not be made");
            Refusal::FolderNotPermitted
        })?;
        let env = login::with_folder_of(self.env.clone(), path);
        let mut env = super::private_tmp::with_tmp(env, &tmp);
        if new.program == Program::ClaudeCode {
            env.extend(
                login::CLAUDE_VARIABLES.map(|(name, value)| (name.to_owned(), value.to_owned())),
            );
        }
        let row = ThreadRow {
            id,
            program: new.program,
            folder: folder.clone(),
            mode: new.mode,
            model: model.as_ref().map(|model| model.as_str().to_owned()),
            state: RunState::Starting,
            session: None,
            title: String::new(),
            started: chrono::Utc::now(),
            ended: None,
            open: Vec::new(),
            queued: Vec::new(),
            last_turn: None,
            effort: new.effort,
            speed: new.speed.clone().filter(|speed| speed != STANDARD),
            workspace: Some(Workspace::of(&folder)),
            usage: None,
        };
        let action = wire::Action::Start {
            thread: id.to_string(),
            program: invocation.program.to_string_lossy().into_owned(),
            args: invocation.args,
            env,
            folder: folder.to_string_lossy().into_owned(),
        };
        Ok((row, action))
    }
}

//! `anima agents set` (TH110, TH117).

use agent_threads::control::Action;
use agent_threads::drive::Effort;
use uuid::Uuid;

use super::{ModeName, parse_effort, simple};
use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(Debug, clap::Args)]
#[command(group = clap::ArgGroup::new("change").required(true).multiple(true).args(["model", "effort", "mode", "speed"]))]
pub struct SetArgs {
    thread: Uuid,
    /// The model of the next turns: an alias or a full name.
    #[arg(long)]
    model: Option<String>,
    /// The effort of the next turns (Codex only: Claude Code refuses it after the start).
    #[arg(long, value_parser = parse_effort)]
    effort: Option<Effort>,
    /// The mode of the next turns (plan is Claude Code only).
    #[arg(long, value_enum)]
    mode: Option<ModeName>,
    /// The speed of the next turns, from `anima agents models`, or `default` (Codex only).
    #[arg(long)]
    speed: Option<String>,
}

impl SetArgs {
    /// # Errors
    /// [`Failure`] for a refusal of the runner.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let action = Action::Set {
            thread: self.thread,
            model: self.model,
            effort: self.effort,
            mode: self.mode.map(ModeName::mode),
            speed: self.speed,
        };
        simple(
            invocation,
            action,
            "Changed. It takes effect from the next turn.",
        )
        .await
    }
}

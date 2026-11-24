//! The provider and permission choices shared by start and set.

use agent_threads::drive::{Mode, Program};

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum ProgramName {
    ClaudeCode,
    Codex,
    Grok,
}

impl ProgramName {
    #[must_use]
    pub const fn program(self) -> Program {
        match self {
            Self::ClaudeCode => Program::ClaudeCode,
            Self::Codex => Program::Codex,
            Self::Grok => Program::Grok,
        }
    }
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum ModeName {
    /// The program asks nothing (the default, R-308).
    FullAccess,
    /// The program changes files in the folder without asking and asks for the rest.
    AcceptEdits,
    /// The program asks before each change.
    Ask,
    /// The program plans and changes nothing until you accept its plan (Claude Code only).
    Plan,
}

impl ModeName {
    pub(super) const fn mode(self) -> Mode {
        match self {
            Self::FullAccess => Mode::FullAccess,
            Self::AcceptEdits => Mode::AcceptEdits,
            Self::Ask => Mode::Ask,
            Self::Plan => Mode::Plan,
        }
    }
}

//! The commands and the skills of a thread (TH117): what its program last named, a skill that a
//! prompt names, and a compact of the context. Pure.

use agent_threads::control::Refusal;
use agent_threads::drive::{ChosenSkill, CommandLists};

use super::{Driver, Step, Thread};

impl Thread {
    /// The lists that the program last named, or `None` while it named none. A Codex thread
    /// with none asks again; the line is in the steps.
    pub fn commands(&mut self) -> (Option<CommandLists>, Vec<Step>) {
        if let Some(lists) = &self.lists {
            return (Some(lists.clone()), Vec::new());
        }
        let again = match &mut self.driver {
            Driver::Codex(codex) if self.held => vec![Step::Write(codex.skills_again())],
            Driver::Codex(_) | Driver::Claude(_) => Vec::new(),
        };
        (None, again)
    }

    /// Whether each skill is one that the program of the thread listed, by its name and its path:
    /// a prompt never names a file that the program did not give (TH117).
    ///
    /// # Errors
    /// [`Refusal::BadValue`] for a skill that is not in the last list, or any skill for Claude
    /// Code, which takes a skill as the text `/name`.
    pub fn takes(&self, skills: &[ChosenSkill]) -> Result<(), Refusal> {
        if skills.is_empty() {
            return Ok(());
        }
        let listed = match (&self.driver, &self.lists) {
            (Driver::Codex(_), Some(lists)) => &lists.skills,
            _ => return Err(Refusal::BadValue),
        };
        let each_listed = skills.iter().all(|chosen| {
            listed
                .iter()
                .any(|skill| skill.name == chosen.name && skill.path == chosen.path)
        });
        if each_listed {
            Ok(())
        } else {
            Err(Refusal::BadValue)
        }
    }

    /// The line that asks Codex to compact the context of the thread.
    ///
    /// # Errors
    /// [`Refusal::NotRunning`] when the program does not run, and [`Refusal::CannotChange`] for
    /// Claude Code (it takes `/compact` as a prompt) and for a Codex thread with no ID yet.
    pub fn compact(&mut self) -> Result<Vec<Step>, Refusal> {
        self.can_take_a_prompt()?;
        let cannot = |said: &str| Refusal::CannotChange {
            said: said.to_owned(),
        };
        match &mut self.driver {
            Driver::Codex(codex) => codex
                .compact()
                .map(|line| vec![Step::Write(line)])
                .ok_or_else(|| cannot("Codex has not started its thread yet: try again soon.")),
            Driver::Claude(_) => Err(cannot(
                "Claude Code compacts with the prompt /compact: send it as a prompt.",
            )),
        }
    }
}

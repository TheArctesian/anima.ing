//! The commands and the skills of a thread (TH117): what its program last named, a skill that a
//! prompt names, and a compact of the context. Pure.

use std::time::{Duration, Instant};

use agent_threads::control::Refusal;
use agent_threads::drive::{ChosenSkill, CommandLists, CommandRow};

use super::{Driver, Step, Thread};

/// How long the skills of a Codex thread stay fresh, and how soon a thread with none asks again.
const FRESH: Duration = Duration::from_secs(60);
const AGAIN: Duration = Duration::from_secs(10);
/// The most skills that one prompt names.
const MOST_SKILLS: usize = 16;

impl Thread {
    /// The lists that the program last named at `now`, with the commands that the runner runs
    /// for it (Codex `compact`), or `None` while it named none. A Codex thread asks again for
    /// its skills when it has none for 10 seconds, or a list older than a minute; the line is in
    /// the steps, and the list comes with a later call.
    pub fn commands(&mut self, now: Instant) -> (Option<CommandLists>, Vec<Step>) {
        let wait = if self.lists.is_some() { FRESH } else { AGAIN };
        let due = self
            .lists_asked
            .is_none_or(|asked| now.saturating_duration_since(asked) >= wait);
        let again = match &mut self.driver {
            Driver::Codex(codex) if self.held && due && codex.thread.is_some() => {
                self.lists_asked = Some(now);
                vec![Step::Write(codex.skills_again())]
            }
            Driver::Codex(_) | Driver::Claude(_) | Driver::Grok(_) => Vec::new(),
        };
        let lists = self.lists.clone().map(|mut lists| {
            if matches!(self.driver, Driver::Codex(_)) {
                lists.commands.insert(0, CommandRow::compact());
            }
            lists
        });
        (lists, again)
    }

    /// Whether each skill is one that the program of the thread listed, by its name and its path:
    /// a prompt never names a file that the program did not give (TH117). A prompt names at most
    /// 16 skills, each once.
    ///
    /// # Errors
    /// [`Refusal::BadValue`] for a skill that is not in the last list, or any skill for Claude
    /// Code, which takes a skill as the text `/name`.
    pub fn takes(&self, skills: &[ChosenSkill]) -> Result<(), Refusal> {
        if skills.is_empty() {
            return Ok(());
        }
        let once = skills
            .iter()
            .enumerate()
            .all(|(at, chosen)| !skills[..at].contains(chosen));
        if skills.len() > MOST_SKILLS || !once {
            return Err(Refusal::BadValue);
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
            Driver::Grok(_) => Err(cannot("Grok compacts with /compact: send it as a prompt.")),
            Driver::Claude(_) => Err(cannot(
                "Claude Code compacts with the prompt /compact: send it as a prompt.",
            )),
        }
    }
}

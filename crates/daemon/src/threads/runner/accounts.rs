//! The accounts of the programs of this computer, as the programs said them (THR-132): in the
//! memory of the runner only. They are not saved, not logged, not in an update of a thread and
//! not sent to the hub in stage 1b (THR-051). Beside them, the models that each program offers
//! (TH104), and what a start may name of them (TH117). Pure.

use agent_threads::control::{AccountRow, ModelRow};
#[cfg(test)]
use agent_threads::drive::SpeedChoice;
use agent_threads::drive::{AccountFacts, Effort, ModelChoice, Program};
use chrono::{DateTime, Utc};

/// What the programs said of themselves for the runner only: their accounts and their models.
#[derive(Debug, Default)]
pub struct Said {
    pub accounts: Accounts,
    pub models: Models,
}

/// The newest account of each program.
#[derive(Debug, Default)]
pub struct Accounts {
    rows: Vec<AccountRow>,
}

impl Accounts {
    /// A report of a program at `at`, merged over its earlier reports.
    pub fn note(&mut self, program: Program, facts: AccountFacts, at: DateTime<Utc>) {
        if let Some(row) = self.rows.iter_mut().find(|row| row.program == program) {
            row.facts.merge(facts);
            row.at = at;
        } else {
            self.rows.push(AccountRow { program, facts, at });
        }
    }

    #[must_use]
    pub fn rows(&self) -> Vec<AccountRow> {
        self.rows.clone()
    }
}

/// The newest list of models of each program (TH104): what a start can name. In memory only,
/// as the accounts: a program names its models when one of its threads starts, or when the
/// runner asks it before any thread (`models.rs`).
#[derive(Debug, Default)]
pub struct Models {
    rows: Vec<ModelRow>,
}

impl Models {
    /// The list a program gave at `at`, in place of its earlier list.
    pub fn note(&mut self, program: Program, models: Vec<ModelChoice>, at: DateTime<Utc>) {
        self.rows.retain(|row| row.program != program);
        self.rows.push(ModelRow {
            program,
            models,
            at,
        });
    }

    #[must_use]
    pub fn rows(&self) -> Vec<ModelRow> {
        self.rows.clone()
    }

    /// Whether `program` may take what a start or a change names. The efforts every program has
    /// always taken pass as before, and so does the standard speed; `none`, `minimal`, `ultra`
    /// and each other speed pass only for a named model whose program listed them (TH110,
    /// TH117).
    #[must_use]
    pub fn offers(&self, program: Program, asked: &Asked<'_>) -> bool {
        let effort = asked.effort.filter(|effort| !effort.common());
        let speed = asked.speed.filter(|speed| *speed != STANDARD);
        if effort.is_none() && speed.is_none() {
            return true;
        }
        let Some(model) = asked.model else {
            return false;
        };
        self.rows
            .iter()
            .filter(|row| row.program == program)
            .flat_map(|row| &row.models)
            .filter(|choice| choice.value == model)
            .any(|choice| {
                effort.is_none_or(|effort| choice.efforts.iter().any(|e| e == effort.as_str()))
                    && speed.is_none_or(|speed| choice.speeds.iter().any(|s| s.id == speed))
            })
    }
}

/// The speed that every model has: Codex takes it to leave a faster speed (TH117).
pub const STANDARD: &str = "default";

/// What a start or a change names of the model.
#[derive(Clone, Copy, Debug, Default)]
pub struct Asked<'a> {
    pub model: Option<&'a str>,
    pub effort: Option<Effort>,
    pub speed: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn th104_th117_an_effort_or_a_speed_out_of_the_common_ones_needs_a_model_that_lists_it() {
        let choice = |value: &str, efforts: &[&str], speeds: &[&str]| ModelChoice {
            value: value.to_owned(),
            name: value.to_owned(),
            efforts: efforts.iter().map(|effort| (*effort).to_owned()).collect(),
            speeds: speeds
                .iter()
                .map(|id| SpeedChoice {
                    id: (*id).to_owned(),
                    name: (*id).to_owned(),
                    description: None,
                })
                .collect(),
            ..ModelChoice::default()
        };
        let mut models = Models::default();
        let listed = vec![
            choice("gpt-6", &["none", "low", "ultra"], &["fast"]),
            choice("gpt-5", &["low"], &[]),
        ];
        models.note(Program::Codex, listed, Utc::now());
        let asks = |model, effort, speed| Asked {
            model,
            effort,
            speed,
        };
        let codex = |asked| models.offers(Program::Codex, &asked);
        for effort in [Effort::Ultra, Effort::Off] {
            assert!(codex(asks(Some("gpt-6"), Some(effort), None)));
            assert!(!codex(asks(Some("gpt-5"), Some(effort), None)));
            assert!(!codex(asks(None, Some(effort), None)), "no model named");
        }
        assert!(!codex(asks(Some("gpt-6"), Some(Effort::Minimal), None)));
        assert!(codex(asks(Some("gpt-6"), None, Some("fast"))));
        assert!(!codex(asks(Some("gpt-5"), None, Some("fast"))));
        assert!(!codex(asks(
            Some("gpt-6"),
            Some(Effort::Ultra),
            Some("flex")
        )));
        assert!(
            codex(asks(None, None, Some(STANDARD))),
            "the standard speed"
        );
        let ultra = asks(Some("gpt-6"), Some(Effort::Ultra), None);
        assert!(!models.offers(Program::ClaudeCode, &ultra));
        assert!(
            models.offers(Program::ClaudeCode, &asks(None, Some(Effort::Max), None)),
            "as before"
        );
    }
}

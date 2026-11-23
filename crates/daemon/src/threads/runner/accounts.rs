//! The accounts of the programs of this computer, as the programs said them (THR-132): in the
//! memory of the runner only. They are not saved, not logged, not in an update of a thread and
//! not sent to the hub in stage 1b (THR-051). Beside them, the models that each program offers
//! (TH104). Pure.

use agent_threads::control::{AccountRow, ModelRow};
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

    /// Whether `program` may take `effort` for `model`. The efforts every program has always
    /// taken pass as before; `ultra` passes only for a named model whose program listed it.
    #[must_use]
    pub fn offers(&self, program: Program, model: Option<&str>, effort: Option<Effort>) -> bool {
        if effort != Some(Effort::Ultra) {
            return true;
        }
        let Some(model) = model else {
            return false;
        };
        self.rows
            .iter()
            .filter(|row| row.program == program)
            .flat_map(|row| &row.models)
            .any(|choice| choice.value == model && choice.efforts.iter().any(|e| e == "ultra"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn th104_ultra_is_taken_only_for_a_model_whose_program_lists_it() {
        let choice = |value: &str, efforts: &[&str]| ModelChoice {
            value: value.to_owned(),
            name: value.to_owned(),
            efforts: efforts.iter().map(|effort| (*effort).to_owned()).collect(),
            default_effort: None,
        };
        let mut models = Models::default();
        let listed = vec![
            choice("gpt-6", &["low", "ultra"]),
            choice("gpt-5", &["low"]),
        ];
        models.note(Program::Codex, listed, Utc::now());
        let ultra = Some(Effort::Ultra);
        assert!(models.offers(Program::Codex, Some("gpt-6"), ultra));
        assert!(!models.offers(Program::Codex, Some("gpt-5"), ultra));
        assert!(
            !models.offers(Program::Codex, None, ultra),
            "no model named"
        );
        assert!(!models.offers(Program::ClaudeCode, Some("gpt-6"), ultra));
        assert!(
            models.offers(Program::ClaudeCode, None, Some(Effort::Max)),
            "as before"
        );
    }
}

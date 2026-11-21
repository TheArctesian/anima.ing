//! The accounts of the programs of this computer, as the programs said them (THR-132): in the
//! memory of the runner only. They are not saved, not logged, not in an update of a thread and
//! not sent to the hub in stage 1b (THR-051). Beside them, the models that each program offers
//! (TH104). Pure.

use agent_threads::control::{AccountRow, ModelRow};
use agent_threads::drive::{AccountFacts, ModelChoice, Program};
use chrono::{DateTime, Utc};

use super::thread::Step;

/// What the programs said of themselves for the runner only: their accounts and their models.
#[derive(Debug, Default)]
pub struct Said {
    pub accounts: Accounts,
    pub models: Models,
}

impl Said {
    /// Keeps a step [`Step::Account`] or [`Step::Models`] of a thread of `program`, said at `at`.
    /// Each other step is not a fact of a program, and is left.
    pub fn note(&mut self, program: Program, step: Step, at: DateTime<Utc>) {
        match step {
            Step::Account(facts) => self.accounts.note(program, *facts, at),
            Step::Models(models) => self.models.note(program, models, at),
            _ => {}
        }
    }
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
/// as the accounts: a program names its models when one of its threads starts.
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
}

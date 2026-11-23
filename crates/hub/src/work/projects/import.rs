//! The import rule (PRO-001, PRO-002, PRJ-016, SRC-012); the intake carries the step out in the
//! run's transaction.

use super::model::{Canonical, Known};

const MAX_FULL_NAME: usize = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Unknown: a new project with this repository (count: inserted).
    Import,
    /// Known, and a field, its presence or its account changed: write them (count: changed).
    /// A rename is one: the subject key follows it (PRJ-016).
    Refresh,
    /// Left out but marked gone: only clear `gone`, write nothing else (count: changed).
    Return,
    /// Nothing to write (count: unchanged).
    Keep,
}

/// A repository whose account was removed with its records kept is adopted by the account that
/// lists it; one that another account already holds stays with it.
#[must_use]
pub fn step(known: Option<&Known>, fields: &Canonical) -> Step {
    let Some(known) = known else {
        return Step::Import;
    };
    if known.left_out {
        return if known.gone { Step::Return } else { Step::Keep };
    }
    if known.gone || known.fields != *fields || known.account_id.is_none() {
        Step::Refresh
    } else {
        Step::Keep
    }
}

/// The text is otherwise kept as the forge gives it.
#[must_use]
pub fn usable(full_name: &str) -> bool {
    full_name.len() <= MAX_FULL_NAME
        && full_name
            .split_once('/')
            .is_some_and(|(owner, name)| !owner.is_empty() && !name.is_empty())
}

#[must_use]
pub fn project_name(full_name: &str) -> &str {
    full_name
        .split_once('/')
        .map_or(full_name, |(_, name)| name)
}

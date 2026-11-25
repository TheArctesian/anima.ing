//! Remote Code launch and command admission (K10 Section 6.5).

mod admission;
mod attempt;
mod audit;
mod commands;
mod control;
mod grant;
mod model;
mod store;
mod sweep;

pub(in crate::fleet) use admission::{launch_receipt, start, targets};
pub(in crate::fleet) use attempt::{candidate, claim, dispatch, evidence, replace_targets, report};
pub(in crate::fleet) use audit::Actor;
pub(in crate::fleet) use commands::{Payload, admit, command_receipt, poll, result};
pub(in crate::fleet) use control::{control, upload};
pub(in crate::fleet) use model::*;

#[derive(Debug, thiserror::Error)]
pub(in crate::fleet) enum Error {
    #[error("{0}")]
    Refused(&'static str),
    #[error("remote thread store error")]
    Database(#[from] sqlx::Error),
    #[error("work state error")]
    Jobs(crate::work::jobs::Error),
    #[error("remote thread audit error")]
    Audit(crate::fleet::events::Error),
    #[error("remote thread wire error")]
    Encoding(#[from] serde_json::Error),
}

impl Error {
    fn malformed() -> Self {
        Self::Refused("MALFORMED")
    }
}

pub(in crate::fleet::agent_threads) use attempt::published;
pub(in crate::fleet) use store::{clear_offers, operator};
pub(in crate::fleet::agent_threads) use sweep::sweep;

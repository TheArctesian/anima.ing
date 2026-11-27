//! The one program of Anima that talks to Docker (contract K10 Section 6.4).
//!
//! The planner decides over facts supplied by the caller. Linux execution lives in its own module.
//! The program does not trust the daemon or the hub: where the contract is not exact, it refuses.

#[cfg(target_os = "linux")]
pub mod control;
#[cfg(target_os = "linux")]
pub mod execution;
pub mod final_form;
mod forms;
mod inventory;
mod json;
#[cfg(target_os = "linux")]
pub mod migration;
mod pair;
pub mod plan;
mod refusal;
pub mod request;
pub mod settings;
pub mod start;
#[cfg(target_os = "linux")]
pub mod startup;
mod strict;
pub mod work;

#[cfg(test)]
mod tests;

pub use final_form::{FinalForm, Frame};
pub use inventory::{Container, Image, Installed, Inventory, Network, Pulled, Records, Volume};
pub use pair::{Pair, PairDecision, PairFile, PairRecord, decided as pair_decided};
pub use plan::{Command, Folders, Paths, Plan, Step};
pub use refusal::Refusal;
pub use request::{Action, AdoptedAction, Apply, Pruned, Removal, Request, Work};
pub use settings::{EngineKind, Settings};

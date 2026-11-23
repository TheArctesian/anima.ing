//! The one program of Anima that talks to Docker (contract K10 Section 6.4).
//!
//! The caller gives each fact of the machine as a value; nothing here reads the engine, writes a file or starts a program.
//! The program does not trust the daemon or the hub: where the contract is not exact, it refuses.

pub mod final_form;
mod forms;
mod inventory;
mod json;
mod pair;
pub mod plan;
mod refusal;
pub mod request;
pub mod settings;
pub mod start;
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

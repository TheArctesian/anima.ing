//! The pure part of model work at the machine (`contracts/compute/model-work.md`).

pub mod cause;
pub mod command;
pub mod drain;
pub mod files;
pub mod folders;
pub mod found;
pub mod launcher;
pub mod loopback;
pub mod machine;
pub mod memory;
pub mod names;
pub mod order;
pub mod pull;
pub mod releases;
pub mod settings;
pub mod supervision;
#[cfg(test)]
mod tests;

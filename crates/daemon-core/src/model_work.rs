//! The pure part of model work at the machine (`contracts/compute/model-work.md`).

pub mod cause;
pub mod command;
pub mod drain;
pub mod files;
pub mod folders;
pub mod loopback;
pub mod machine;
pub mod names;
pub mod order;
pub mod releases;
pub mod settings;
pub mod supervision;
#[cfg(test)]
mod tests;

//! The I/O edge of model work at the machine (`contracts/compute/model-work.md`); the pure
//! checks are in `daemon_core::model_work`.

pub mod cycle;
pub mod drain;
pub mod fetch;
pub mod folders;
pub mod found;
pub mod install;
pub mod load;
pub mod machine_files;
pub mod modes;
pub mod pull;
pub mod reports;
pub mod ring;
pub mod server;
pub mod sockets;
pub mod state;
pub mod supervise;
pub mod tree;
pub mod unpack;

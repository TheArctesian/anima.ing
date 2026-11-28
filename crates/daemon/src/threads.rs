//! Agent threads on this computer (stage 1b of `docs/proposals/agent-threads.md`): the holder
//! that owns the agent programs (THR-113), and the link of the daemon to it.
//!
//! Pure: [`wire`] (the frames), [`outbox`] (the lines that wait for the daemon) and the walk of
//! [`tree`]. I/O: [`holder`] with [`commands`] and [`program`] (the holder process), [`record`]
//! (its file) and [`link`] (the daemon side of the socket); [`runner`] drives the programs;
//! [`git`] is the Git of a thread (its worktree, its checkpoints, its commit and its push).

pub mod commands;
pub mod git;
pub mod holder;
pub mod holder_channel;
pub mod link;
pub mod marker;
pub mod notice;
pub mod outbox;
pub mod priority;
pub mod program;
pub mod record;
pub mod runner;
pub mod stop;
pub mod tree;
pub mod wire;

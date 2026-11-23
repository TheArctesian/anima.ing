//! The terminal tunnel (K4 Section 8, R-062): the hub runs the SSH connection itself, with the
//! accepted host key pinned (SEC-007), and no SSH session goes to guacd.

mod control;
mod ending;
mod link;
mod opening;
mod relay;
mod route;
mod session;
mod shell;
#[cfg(test)]
mod tests;
mod writes;

pub use route::terminal_router;

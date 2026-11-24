//! The work route at the machine (`contracts/work-route.md`, K10): the pure decision before the
//! daemon starts a job that the hub gave it. Nothing here asks the hub for work or starts a program.

pub mod checks;
pub mod permitted;
pub mod record;
pub mod refusal;
mod shape;
pub mod switch;
#[cfg(test)]
mod tests;
pub mod types;

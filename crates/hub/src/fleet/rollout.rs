//! The pure rules of a release rollout (ROL-023 to ROL-028, ROL-039 to ROL-041; K8 Section 8).
//! Nothing here reads a clock, a database or the network (CQ-021).

mod instruction;
mod membership;
mod model;
mod path;
mod progress;
mod state;
mod steps;
#[cfg(test)]
mod tests;

pub use instruction::{Aim, Instruction, instruction};
pub use membership::{LeftOut, place};
pub use model::{Cause, Machine, Outcome, Reach, Release, Said, Target, Try};
pub use path::next_hop;
pub use progress::{Next, Pace, Rollout, Standing, Step, next, standing};
pub use state::{Aimed, State, state};
pub use steps::{first_machine, step_size, steps};

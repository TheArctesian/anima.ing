//! What the hub tells one machine in the answer to a push (K8 Section 8.2, ROL-009).

use super::{
    model::{Machine, Outcome, Reach, Release, Try},
    path::next_hop,
};
use crate::fleet::builds::Version;

/// The most tries of one version without the operator (ROL-032).
const MOST_TRIES: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aim {
    /// The newest rollout whose step for the machine started (ROL-040); `again` after "Try again" (ROL-032).
    Forward { version: Version, again: bool },
    /// "Roll back": the previous version of the machine (ROL-027, ROL-031).
    Back,
}

/// The `update` member of an `Accepted`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub version: Version,
    pub again: bool,
}

fn ended(last: Option<Try>, version: Version, again: bool) -> bool {
    last.filter(|tried| tried.version == version)
        .is_some_and(|tried| match tried.outcome {
            Outcome::Installed => false,
            Outcome::Refused(_) => !again,
            Outcome::RolledBack(_) => !again && tried.tries >= MOST_TRIES,
        })
}

/// The hub repeats the same instruction with each push until this returns `None`.
#[must_use]
pub fn instruction(machine: &Machine, aim: Aim, held: &[Release]) -> Option<Instruction> {
    let said = machine.said.filter(|said| said.switch_on)?;
    if machine.held || machine.reach == Reach::TokenEnded {
        return None;
    }
    let installed = machine.installed?;
    match aim {
        Aim::Back => {
            let version = said.previous.filter(|previous| *previous != installed)?;
            Some(Instruction {
                version,
                again: false,
            })
        }
        Aim::Forward { version, again } => {
            let hop = next_hop(installed, said.target?, version, held)?;
            (!ended(said.last, hop, again)).then_some(Instruction {
                version: hop,
                again,
            })
        }
    }
}

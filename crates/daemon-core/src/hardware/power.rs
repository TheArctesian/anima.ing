//! Where the power comes from (CAP-004), as the batch's `PowerSource`; the parse is in the crate `power`.

pub use ::power::{Power, Supply, from_pmset, from_supplies};

use super::PowerSource;

impl From<Power> for PowerSource {
    fn from(power: Power) -> Self {
        match power {
            Power::Mains => Self::Mains,
            Power::Battery { .. } => Self::Battery,
        }
    }
}

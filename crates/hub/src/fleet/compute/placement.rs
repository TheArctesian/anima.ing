//! Where a model can run: the fit estimate (FIT-001 to FIT-005, CMP-011), and its inputs for one
//! variant on one machine (task L1-20).

mod fit;
mod machine;
#[cfg(test)]
mod machine_tests;
mod overhead;
#[cfg(test)]
mod tests;

pub use fit::{FitError, FitRequest, FitVerdict, KvShape, estimate_fit};
pub use machine::{DEFAULT_CONTEXT, Estimate, Machine, Placement, Unknown, estimate};
pub use overhead::{OverheadEntry, OverheadTable};

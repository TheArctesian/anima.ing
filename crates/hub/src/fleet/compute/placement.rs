//! Where a model can run: the fit estimate (FIT-001 to FIT-005, CMP-011).

mod fit;
mod overhead;
#[cfg(test)]
mod tests;

pub use fit::{FitError, FitRequest, FitVerdict, KvShape, estimate_fit};
pub use overhead::{OverheadEntry, OverheadTable};

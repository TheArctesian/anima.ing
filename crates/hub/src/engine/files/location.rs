//! Location metadata of a file (FIL-003): read at upload when kept, removed from each copy sent.

mod exif;
mod jpeg;
#[cfg(test)]
mod tests;

pub use exif::Position;
pub use jpeg::{CLEANED_TYPES, Cleaner, is_jpeg};

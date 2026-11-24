//! Who may have written the settings file (contract K10 Section 6.4.2): the identifier 0 owns
//! it, and neither a group nor others can write it.

/// What the operating system says of the settings file, read without following a link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Guarded {
    pub owner: u32,
    pub mode: u32,
    /// True for a regular file; false for a link, a folder and each other thing.
    pub regular: bool,
}

/// Why the settings file is not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NotGuarded {
    #[error("the settings file is not a regular file")]
    NotRegular,
    #[error("the identifier 0 does not own the settings file")]
    Owner,
    #[error("a group or others can write the settings file")]
    Writable,
}

/// Decides if the settings file can be trusted to be of the administrator.
///
/// # Errors
/// The reason the file is not trusted.
pub const fn guard(file: &Guarded) -> Result<(), NotGuarded> {
    if !file.regular {
        return Err(NotGuarded::NotRegular);
    }
    if file.owner != 0 {
        return Err(NotGuarded::Owner);
    }
    if file.mode & 0o022 != 0 {
        return Err(NotGuarded::Writable);
    }
    Ok(())
}

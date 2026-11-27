//! The private input and output records of one checked Compose version.

use zeroize::Zeroize as _;

use super::Folders;

/// One candidate version, including the generated form passed to Compose.
#[derive(Clone, PartialEq, Eq)]
pub struct Version {
    pub(crate) folders: Folders,
    pub(crate) keep_previous: bool,
    pub(crate) compose: Vec<u8>,
    pub(crate) environment: Vec<u8>,
    pub(crate) digests: Vec<u8>,
    pub(crate) final_form: Vec<u8>,
}

impl Drop for Version {
    fn drop(&mut self) {
        self.compose.zeroize();
        self.environment.zeroize();
        self.final_form.zeroize();
    }
}

impl std::fmt::Debug for Version {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Version")
            .field("folders", &self.folders)
            .field("keep_previous", &self.keep_previous)
            .finish_non_exhaustive()
    }
}

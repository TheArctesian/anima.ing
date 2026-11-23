use serde::{Serialize, Serializer, ser::SerializeStruct};

/// Carries no path and no OS error text: the path names the operator's home folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProcessLogError {
    #[error("the log folder cannot be opened on this system")]
    Unavailable,
    #[error("the log folder did not open")]
    Failed,
}

impl ProcessLogError {
    /// Stable code the client switches on.
    pub(super) fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Failed => "failed",
        }
    }
}

/// Serialises as `{ "code": ..., "message": ... }` with the fixed `Display` text.
impl Serialize for ProcessLogError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("ProcessLogError", 2)?;
        out.serialize_field("code", self.code())?;
        out.serialize_field("message", &self.to_string())?;
        out.end()
    }
}

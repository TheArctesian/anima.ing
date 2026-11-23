use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lid {
    Ready,
    #[cfg(unix)]
    NeedsSetUp,
    /// The desktop decides the lid itself and ignores another program (KDE Plasma).
    Unsupported,
}

/// A cancelled password dialog is `Refused` too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SetUpError {
    #[error("this system has no set-up step")]
    Unsupported,
    #[error("the administrator step was cancelled or failed")]
    Refused,
}

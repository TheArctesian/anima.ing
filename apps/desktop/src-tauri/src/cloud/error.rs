use serde::{Serialize, Serializer, ser::SerializeStruct};

/// Carries no token, code or answer text (LOG-007); the client switches on the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CloudError {
    /// `ANIMA_CLOUD_CLIENT_ID` was empty when this build was made.
    #[error("this build has no Anima.ing sign-in")]
    Unconfigured,
    #[error("only the main window can do this")]
    Caller,
    /// `access_denied`.
    #[error("the sign-in was cancelled")]
    Cancelled,
    /// `expired_token`, or no sign-in was started.
    #[error("the code expired")]
    Expired,
    /// No answer, or 5xx.
    #[error("the cloud did not answer")]
    Offline,
    #[error("the cloud refused")]
    Refused,
    #[error("sign in again")]
    Unauthenticated,
    /// 402 `payment_failed`: the cloud makes and wakes nothing.
    #[error("the payment failed")]
    PaymentFailed,
    /// 409 `not_ready`.
    #[error("the hub is not ready")]
    NotReady,
    #[error("the key store refused")]
    Keystore,
}

impl CloudError {
    /// Stable code the client switches on.
    pub(super) fn code(self) -> &'static str {
        match self {
            Self::Unconfigured => "unconfigured",
            Self::Caller => "caller",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
            Self::Offline => "offline",
            Self::Refused => "refused",
            Self::Unauthenticated => "unauthenticated",
            Self::PaymentFailed => "payment-failed",
            Self::NotReady => "not-ready",
            Self::Keystore => "keystore",
        }
    }
}

/// Serialises as `{ "code": ... }` and nothing else (WP-31 "Shell interface").
impl Serialize for CloudError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("CloudError", 1)?;
        out.serialize_field("code", self.code())?;
        out.end()
    }
}

impl From<reqwest::Error> for CloudError {
    /// Its text, which can quote the URL, is dropped.
    fn from(_: reqwest::Error) -> Self {
        Self::Offline
    }
}

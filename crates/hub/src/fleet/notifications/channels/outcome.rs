use super::{Secrets, Settings};
use crate::api_errors::FieldError;

const UNOPENED: &str = "The stored secrets cannot be opened. Enter every secret again.";

pub(super) fn unopened() -> Vec<FieldError> {
    vec![FieldError::new("settings", UNOPENED)]
}

pub(in crate::fleet::notifications) enum Kept<T> {
    Nothing,
    Opened(T),
    Unopened,
}

impl<T> Kept<T> {
    /// A value of the wrong shape counts as unopened.
    pub(super) fn map<U>(self, convert: impl FnOnce(T) -> Option<U>) -> Kept<U> {
        match self {
            Self::Nothing => Kept::Nothing,
            Self::Opened(value) => convert(value).map_or(Kept::Unopened, Kept::Opened),
            Self::Unopened => Kept::Unopened,
        }
    }

    /// The settings said secrets exist, so nothing stored, or nothing that opens, asks for every secret again.
    pub(super) fn opened(self) -> Result<T, Vec<FieldError>> {
        match self {
            Self::Opened(value) => Ok(value),
            Self::Nothing | Self::Unopened => Err(unopened()),
        }
    }
}

/// The secret fields a write set or cleared, by input field name, never by value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::fleet::notifications) struct Touched {
    pub(in crate::fleet::notifications) set: Vec<&'static str>,
    pub(in crate::fleet::notifications) cleared: Vec<&'static str>,
}

impl Touched {
    pub(super) fn set(&mut self, field: &'static str) {
        self.set.push(field);
    }

    pub(super) fn cleared(&mut self, field: &'static str) {
        self.cleared.push(field);
    }
}

pub(super) struct Outcome {
    pub(super) settings: Settings,
    pub(super) secrets: Option<Secrets>,
    pub(super) touched: Touched,
}

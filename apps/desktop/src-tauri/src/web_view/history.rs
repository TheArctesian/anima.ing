//! WEB-005.

use super::WebViewError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Back,
    Forward,
    Reload,
}

impl Step {
    pub fn parse(text: &str) -> Result<Self, WebViewError> {
        match text {
            "BACK" => Ok(Self::Back),
            "FORWARD" => Ok(Self::Forward),
            "RELOAD" => Ok(Self::Reload),
            _ => Err(WebViewError::Step),
        }
    }

    /// No part of a script comes from input.
    pub fn script(self) -> Option<&'static str> {
        match self {
            Self::Back => Some("history.back()"),
            Self::Forward => Some("history.forward()"),
            Self::Reload => None,
        }
    }
}

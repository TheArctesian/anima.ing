//! The web view names no pane or URL; the command opens one fixed pane (R-059, R-079).

use serde::{Serialize, Serializer, ser::SerializeStruct};
use tauri_plugin_log::log;

use crate::os_program::{self, OsProgramError};

/// General → Sharing, macOS 13 and later.
const SHARING_PANE: &str = "x-apple.systempreferences:com.apple.Sharing-Settings.extension";

/// The URL opener. An absolute path, so `PATH` cannot swap it.
const OPENER: &str = "/usr/bin/open";

/// Carries no OS error text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SystemSettingsError {
    #[error("these settings cannot be opened on this system")]
    Unavailable,
    #[error("the settings did not open")]
    Failed,
}

impl SystemSettingsError {
    /// Stable code the client switches on.
    fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Failed => "failed",
        }
    }
}

/// Serialises as `{ "code": ..., "message": ... }` with the fixed `Display` text.
impl Serialize for SystemSettingsError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("SystemSettingsError", 2)?;
        out.serialize_field("code", self.code())?;
        out.serialize_field("message", &self.to_string())?;
        out.end()
    }
}

fn open_with(opener: &str, target: &str) -> Result<(), SystemSettingsError> {
    os_program::open(opener, target.as_ref()).map_err(|error| match error {
        OsProgramError::NotFound => SystemSettingsError::Unavailable,
        _ => SystemSettingsError::Failed,
    })
}

#[tauri::command]
pub async fn sharing_settings_open() -> Result<(), SystemSettingsError> {
    if !cfg!(target_os = "macos") {
        return Err(SystemSettingsError::Unavailable);
    }
    let opened = tauri::async_runtime::spawn_blocking(|| open_with(OPENER, SHARING_PANE))
        .await
        .map_err(|_| SystemSettingsError::Failed)
        .flatten();
    if let Err(error) = opened {
        log::warn!("the Sharing settings did not open: {}", error.code());
    }
    opened
}

#[cfg(test)]
mod tests {
    use super::{SystemSettingsError, open_with};

    #[test]
    fn an_opener_that_succeeds_opens_the_pane() {
        assert_eq!(
            open_with("true", "x-apple.systempreferences:anything"),
            Ok(())
        );
    }

    #[test]
    fn an_opener_that_fails_is_reported_as_failed() {
        assert_eq!(
            open_with("false", "x-apple.systempreferences:anything"),
            Err(SystemSettingsError::Failed)
        );
    }

    #[test]
    fn a_missing_opener_is_reported_as_unavailable() {
        assert_eq!(
            open_with("/nonexistent/anima-opener", "x"),
            Err(SystemSettingsError::Unavailable)
        );
    }

    #[test]
    fn the_error_crosses_to_the_client_as_a_code_and_fixed_text() {
        let json = serde_json::to_value(SystemSettingsError::Failed).unwrap();
        assert_eq!(json["code"], "failed");
        assert_eq!(json["message"], "the settings did not open");
    }
}

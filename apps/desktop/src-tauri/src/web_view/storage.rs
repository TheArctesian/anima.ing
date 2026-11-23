//! One cookie and site store per device, never the main window's (D-2, WEB-006). Pure: no I/O.

use uuid::Uuid;

use super::WebViewError;

#[cfg(any(test, not(target_os = "macos")))]
pub const STORE_FOLDER: &str = "web";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceId(Uuid);

impl DeviceId {
    /// The hub's hyphenated form only, so one device has one store; the nil UUID names no device.
    pub fn parse(text: &str) -> Result<Self, WebViewError> {
        let canonical = text.len() == 36 && text.as_bytes().get(8) == Some(&b'-');
        let uuid = Uuid::try_parse(text).map_err(|_| WebViewError::DeviceId)?;
        if canonical && !uuid.is_nil() {
            Ok(Self(uuid))
        } else {
            Err(WebViewError::DeviceId)
        }
    }

    /// The `WKWebsiteDataStore` identifier.
    #[cfg(any(test, target_os = "macos"))]
    pub fn store_identifier(self) -> [u8; 16] {
        *self.0.as_bytes()
    }

    /// Hex digits and hyphens only, so it cannot leave its parent folder.
    #[cfg(any(test, not(target_os = "macos")))]
    pub fn folder_name(self) -> String {
        self.0.hyphenated().to_string()
    }
}

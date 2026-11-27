//! The arguments of `web_view_open`, checked field by field before any web view exists.

use serde::Deserialize;
use tauri::Rect;

use super::{
    Fingerprint, WebViewError,
    address::{Embedding, WebUrl, embedding, require_preview_url},
    bounds::Bounds,
    storage::DeviceId,
    tabs::{Tab, TabId},
};

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Opening {
    tab_id: String,
    device_id: String,
    url: String,
    pin: Option<String>,
    preview_endpoint: Option<uuid::Uuid>,
    bounds: Bounds,
}

#[derive(Debug, Clone)]
pub struct NewTab {
    pub id: TabId,
    pub device: DeviceId,
    pub rect: Rect,
    pub tab: Tab,
}

impl Opening {
    /// A wrong shape is `arguments`, whatever the field, so no value is echoed back.
    pub fn read(body: &serde_json::Value) -> Result<Self, WebViewError> {
        Self::deserialize(body).map_err(|_| WebViewError::Arguments)
    }

    pub fn check(self, pinning: bool) -> Result<NewTab, WebViewError> {
        let id = TabId::parse(&self.tab_id)?;
        let device = DeviceId::parse(&self.device_id)?;
        let url = WebUrl::parse(&self.url)?;
        let pin = self
            .pin
            .as_deref()
            .map(str::parse::<Fingerprint>)
            .transpose()?;
        if let Some(endpoint) = self.preview_endpoint {
            require_preview_url(url.as_url(), endpoint, "/.anima/connect")?;
            if pin.is_some() {
                return Err(WebViewError::Arguments);
            }
        }
        let rect = self.bounds.rect()?;
        let (origin, pin) = match embedding(&url, pin, pinning)? {
            Embedding::Plain => (None, None),
            Embedding::Pinned { origin, pin } => (Some(origin), pin),
        };
        let tab = Tab {
            url,
            origin,
            pin,
            preview_endpoint: self.preview_endpoint,
            popped_out: false,
        };
        Ok(NewTab {
            id,
            device,
            rect,
            tab,
        })
    }
}

//! The certificate trust hook of a device web view (D-4); the platform part is the only `unsafe` code.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use crate::event::{Event, Events};
use crate::pin::{self, Pin, Presented, Verdict};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
pub use linux::install;
#[cfg(target_os = "macos")]
pub use macos::install;

pub type Pins = Arc<Mutex<HashMap<String, Pin>>>;

#[derive(Clone)]
pub struct TrustHook {
    label: String,
    pins: Pins,
    events: Events,
}

impl TrustHook {
    pub fn new(label: &str, pins: Pins, events: Events) -> Self {
        Self {
            label: label.to_owned(),
            pins,
            events,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    /// The lock is held only to copy the pin.
    pub fn judge(&self, presented: &Presented) -> Verdict {
        let pin = self
            .pins
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&self.label)
            .cloned();
        let verdict = pin::decide(pin.as_ref(), presented);
        self.events.send(Event::Challenge {
            label: self.label.clone(),
            host: presented.host.clone(),
            port: presented.port,
            verdict: verdict.clone(),
            system_trusted: presented.system_trusted,
        });
        verdict
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn install(_: &tauri::webview::PlatformWebview, _: TrustHook) -> Result<String, String> {
    Err("no trust hook on this platform".into())
}

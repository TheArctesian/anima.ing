//! The `web-view` event (WP-31): it goes to the main web view only, and its URL is never logged (LOG-007).

use serde::Serialize;
use tauri::{Emitter, EventTarget, Runtime, Url};
use tauri_plugin_log::log;

use super::{Fingerprint, Verdict, address::is_web, tabs::TabId};

pub const EVENT: &str = "web-view";

pub const MAIN: &str = "main";

/// Why a web view could not show a page: a short code, never a URL or page text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    Navigate,
}

/// One `web-view` event payload: `{ tabId, kind, ... }`, exactly the client's fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum WebViewEvent {
    Loading {
        tab_id: String,
        url: String,
    },
    Loaded {
        tab_id: String,
        url: String,
    },
    #[cfg_attr(
        not(any(target_os = "macos", test)),
        expect(
            dead_code,
            reason = "only macOS has a trust hook until OD-7 runs on Linux"
        )
    )]
    Untrusted {
        tab_id: String,
        fingerprint_sha256: Fingerprint,
    },
    #[cfg_attr(
        not(any(target_os = "macos", test)),
        expect(
            dead_code,
            reason = "only macOS has a trust hook until OD-7 runs on Linux"
        )
    )]
    Mismatch {
        tab_id: String,
        expected: Fingerprint,
        presented: Fingerprint,
    },
    Failed {
        tab_id: String,
        reason: Failure,
    },
}

impl WebViewEvent {
    pub fn page(tab: &TabId, url: &Url, finished: bool) -> Option<Self> {
        if !is_web(url) {
            return None;
        }
        let (tab_id, url) = (tab.as_str().to_owned(), url.to_string());
        Some(if finished {
            Self::Loaded { tab_id, url }
        } else {
            Self::Loading { tab_id, url }
        })
    }

    #[cfg_attr(
        not(any(target_os = "macos", test)),
        expect(
            dead_code,
            reason = "only macOS has a trust hook until OD-7 runs on Linux"
        )
    )]
    pub fn refusal(tab: &TabId, verdict: Verdict) -> Option<Self> {
        let tab_id = tab.as_str().to_owned();
        match verdict {
            Verdict::Untrusted(fingerprint_sha256) => Some(Self::Untrusted {
                tab_id,
                fingerprint_sha256,
            }),
            Verdict::Mismatch {
                expected,
                presented,
            } => Some(Self::Mismatch {
                tab_id,
                expected,
                presented,
            }),
            Verdict::Accept | Verdict::Default => None,
        }
    }
}

/// A `Webview` target would not reach the client, which listens on the `WebviewWindow` `main`; device web views cannot listen (D-3).
pub fn send<R: Runtime>(emitter: &impl Emitter<R>, event: &WebViewEvent) {
    if emitter
        .emit_to(EventTarget::webview_window(MAIN), EVENT, event)
        .is_err()
    {
        log::warn!("a web-view event was not delivered");
    }
}

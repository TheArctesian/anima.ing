//! Only `http` and `https`, and `https` only where the platform can pin a certificate (D-3, D-6,
//! WEB-007).

use tauri::Url;

use super::{Fingerprint, Pin, PinnedOrigin, WebViewError};

/// The longest address accepted, as the client's own bound.
const URL_MAX: usize = 8192;

pub fn is_web(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

/// Asked for every frame and every new-window request, so a page cannot take its view to an
/// `https` address the view could not pin (D-6).
pub fn may_load(url: &Url, pinning: bool) -> bool {
    is_web(url) && (url.scheme() == "http" || pinning)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebUrl(Url);

impl WebUrl {
    /// The error names the rule broken, never the text.
    pub fn parse(text: &str) -> Result<Self, WebViewError> {
        if text.len() > URL_MAX {
            return Err(WebViewError::Url);
        }
        let url = Url::parse(text).map_err(|_| WebViewError::Url)?;
        if !is_web(&url) {
            return Err(WebViewError::Scheme);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(WebViewError::Userinfo);
        }
        if url.host_str().is_none_or(str::is_empty) {
            return Err(WebViewError::Host);
        }
        Ok(Self(url))
    }

    pub fn is_https(&self) -> bool {
        self.0.scheme() == "https"
    }

    pub fn as_url(&self) -> &Url {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Embedding {
    Plain,
    Pinned {
        origin: PinnedOrigin,
        pin: Option<Pin>,
    },
}

/// `https` needs a platform that pins (otherwise the client opens the system browser); a pin needs
/// `https`.
pub fn embedding(
    url: &WebUrl,
    pin: Option<Fingerprint>,
    pinning: bool,
) -> Result<Embedding, WebViewError> {
    if !url.is_https() {
        return match pin {
            Some(_) => Err(WebViewError::PinScheme),
            None => Ok(Embedding::Plain),
        };
    }
    if !pinning {
        return Err(WebViewError::PinningUnavailable);
    }
    let origin = PinnedOrigin::parse(url.as_str())?;
    let pin = pin.map(|fingerprint| Pin {
        origin: origin.clone(),
        fingerprint,
    });
    Ok(Embedding::Pinned { origin, pin })
}

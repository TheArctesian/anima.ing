//! Checks what the web view asks for before anything runs (MET-019, MET-020, HMD-054).

use std::{fmt, path::Path};

use url::Url;
use zeroize::Zeroizing;

/// K8 Section 3: the prefix, then the unpadded base64url of 16 bytes.
const CODE_PREFIX: &str = "anima_enrol_";
const CODE_TAIL_LENGTH: usize = 22;

/// Goes only to the daemon's standard input; `Debug` hides it and a drop wipes it (LOG-007).
pub struct EnrolmentCode(Zeroizing<String>);

impl EnrolmentCode {
    pub fn parse(text: &str) -> Option<Self> {
        let tail = text.strip_prefix(CODE_PREFIX)?;
        let base64url = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-';
        (tail.len() == CODE_TAIL_LENGTH && tail.bytes().all(base64url))
            .then(|| Self(Zeroizing::new(text.to_owned())))
    }

    pub fn input_line(&self) -> Zeroizing<Vec<u8>> {
        let mut line = Zeroizing::new(Vec::with_capacity(self.0.len() + 1));
        line.extend_from_slice(self.0.as_bytes());
        line.push(b'\n');
        line
    }
}

impl fmt::Debug for EnrolmentCode {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("EnrolmentCode(hidden)")
    }
}

/// `https`, a host and port 443 only (MET-020), in the URL standard's one serialisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubOrigin(String);

impl HubOrigin {
    pub fn parse(text: &str) -> Option<Self> {
        let authority = text.strip_prefix("https://")?;
        let plain = |character: char| {
            !character.is_whitespace() && !character.is_control() && !"/?#@\\".contains(character)
        };
        if authority.is_empty() || !authority.chars().all(plain) {
            return None;
        }
        let url = Url::parse(text).ok()?;
        let bare = url.scheme() == "https"
            && url.host().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none();
        bare.then(|| Self(url.origin().ascii_serialization()))
    }

    /// The socket path is the shell's own, never one from the web view (K8 Section 1, R-172).
    pub fn link(socket: &Path) -> Option<Self> {
        let path = socket.to_str()?;
        let plain = socket.is_absolute() && !path.chars().any(char::is_control);
        plain.then(|| Self(format!("unix:{path}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn link_port(text: &str) -> Option<u16> {
    let port = text.strip_prefix("http://127.0.0.1:")?;
    let digits = !port.is_empty() && port.len() <= 5 && port.bytes().all(|b| b.is_ascii_digit());
    digits
        .then(|| port.parse().ok())
        .flatten()
        .filter(|port| *port != 0)
}

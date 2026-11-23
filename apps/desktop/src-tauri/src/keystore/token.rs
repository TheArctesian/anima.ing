use keyring::Entry;

use super::error::KeystoreError;

pub(super) const RELEASE_SERVICE: &str = "ing.anima.desktop";

/// Debug builds use their own service so they never read or overwrite a released app's token.
pub(crate) const SERVICE: &str = if cfg!(debug_assertions) {
    "ing.anima.desktop.dev"
} else {
    RELEASE_SERVICE
};

/// Kept apart from the operator's access token (BLD-022, BLD-023).
pub(super) const BUILDER_SERVICE: &str = if cfg!(debug_assertions) {
    "ing.anima.desktop.dev.builder"
} else {
    "ing.anima.desktop.builder"
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// The operator's access token (CLI-003).
    Access,
    /// This computer's builder token, which permits only the builder operations (BLD-022).
    Builder,
}

impl TokenKind {
    fn service(self) -> &'static str {
        match self {
            Self::Access => SERVICE,
            Self::Builder => BUILDER_SERVICE,
        }
    }
}

const SCHEME: &str = "https://";
/// Plain `http` is allowed only for `127.0.0.1` (CLI-002, S6 F-1, F-10): `localhost` can resolve
/// outside the loopback owner check.
/// No `[::1]`: the CSP cannot name an IPv6 literal.
const LOOPBACK_SCHEMES: [&str; 1] = ["http://127.0.0.1"];

/// Input check only, no URL resolution (REPO-003): only the canonical origin that `parseHubAddress`
/// produces passes.
fn is_canonical_origin(origin: &str) -> bool {
    let after_scheme = origin.strip_prefix(SCHEME).or_else(|| {
        LOOPBACK_SCHEMES.iter().find_map(|prefix| {
            let rest = origin.strip_prefix(prefix)?;
            is_port_or_nothing(rest).then_some(&origin["http://".len()..])
        })
    });
    after_scheme.is_some_and(|host| {
        !host.is_empty()
            && host
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-_:[]".contains(c))
    })
}

fn is_port_or_nothing(rest: &str) -> bool {
    rest.is_empty()
        || rest.strip_prefix(':').is_some_and(|port| {
            (1..=5).contains(&port.len()) && port.bytes().all(|b| b.is_ascii_digit())
        })
}

/// The account is the hub origin, so a token is only read back for the hub that issued it.
fn entry_for(kind: TokenKind, origin: &str) -> Result<Entry, KeystoreError> {
    if !is_canonical_origin(origin) {
        return Err(KeystoreError::BadOrigin);
    }
    Entry::new(kind.service(), origin).map_err(KeystoreError::from)
}

pub fn read_token(kind: TokenKind, origin: &str) -> Result<Option<String>, KeystoreError> {
    match entry_for(kind, origin)?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn write_token(kind: TokenKind, origin: &str, token: &str) -> Result<(), KeystoreError> {
    let entry = entry_for(kind, origin)?;
    if token.is_empty() {
        return Err(KeystoreError::EmptyToken);
    }
    entry.set_password(token).map_err(KeystoreError::from)
}

pub fn clear_token(kind: TokenKind, origin: &str) -> Result<(), KeystoreError> {
    match entry_for(kind, origin)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

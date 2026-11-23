//! SSH secrets and their pure mapping to stored rows: one sealed row per part (SEC-003).

use crate::ids::DeviceId;

use super::{Error, Secret};

/// Implements no `Debug`, `Display`, `Serialize` or `Clone` (LOG-007, SEC-005).
pub enum SshSecret {
    Password(Secret),
    PrivateKey {
        key: Secret,
        passphrase: Option<Secret>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SshKind {
    Password,
    PrivateKey,
    Passphrase,
}

impl SshKind {
    /// SSH reads, checks and deletes match only these, so another kind's row is never taken for SSH.
    pub(super) const SQL_KINDS: [&'static str; 3] = [
        Self::Password.as_sql(),
        Self::PrivateKey.as_sql(),
        Self::Passphrase.as_sql(),
    ];

    pub(super) const fn as_sql(self) -> &'static str {
        match self {
            Self::Password => "ssh_password",
            Self::PrivateKey => "ssh_private_key",
            Self::Passphrase => "ssh_passphrase",
        }
    }

    pub(super) fn from_sql(text: &str) -> Result<Self, Error> {
        match text {
            "ssh_password" => Ok(Self::Password),
            "ssh_private_key" => Ok(Self::PrivateKey),
            "ssh_passphrase" => Ok(Self::Passphrase),
            _ => Err(Error::Malformed),
        }
    }
}

/// Binds a sealed value to its device and kind, so a value moved to another row does not open.
/// It opens only rows sealed before SEC-011; newer rows use the host-bound context of `host`.
pub(super) fn context(device: DeviceId, kind: SshKind) -> Vec<u8> {
    format!("credentials.secrets:{device}:{}", kind.as_sql()).into_bytes()
}

impl SshSecret {
    pub(super) fn into_parts(self) -> Vec<(SshKind, Secret)> {
        match self {
            Self::Password(password) => vec![(SshKind::Password, password)],
            Self::PrivateKey { key, passphrase } => {
                let mut parts = vec![(SshKind::PrivateKey, key)];
                parts.extend(passphrase.map(|p| (SshKind::Passphrase, p)));
                parts
            }
        }
    }

    pub(super) fn from_parts(
        parts: impl IntoIterator<Item = (SshKind, Secret)>,
    ) -> Result<Self, Error> {
        let (mut password, mut key, mut passphrase) = (None, None, None);
        for (kind, secret) in parts {
            let slot = match kind {
                SshKind::Password => &mut password,
                SshKind::PrivateKey => &mut key,
                SshKind::Passphrase => &mut passphrase,
            };
            if slot.replace(secret).is_some() {
                return Err(Error::Malformed);
            }
        }
        match (password, key, passphrase) {
            (Some(password), None, None) => Ok(Self::Password(password)),
            (None, Some(key), passphrase) => Ok(Self::PrivateKey { key, passphrase }),
            _ => Err(Error::Malformed),
        }
    }
}

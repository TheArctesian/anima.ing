//! The operator App key and the short token it signs (SRC-002, PRJ-018, K-E1 Section 7).
//! Nothing here is `Debug` or logged: a refusal names the field, never the value.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, TimeDelta, Utc};
use ring::{
    rand::SecureRandom,
    signature::{RSA_PKCS1_SHA256, RsaKeyPair},
};
use rustls_pki_types::{PrivateKeyDer, pem::PemObject as _};
use zeroize::Zeroizing;

pub const MAX_PEM_BYTES: usize = 16 * 1024;
const MIN_KEY_BITS: usize = 2048;
const MAX_ID_DIGITS: usize = 20;
/// Set back for a source whose clock is behind.
const ISSUED_BEFORE: TimeDelta = TimeDelta::seconds(60);
/// GitHub accepts at most ten minutes.
const VALID_FOR: TimeDelta = TimeDelta::seconds(540);

/// Why an App key was refused. Names the field, never its value (LOG-007, PRJ-018).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeyRefusal {
    #[error("the App ID must be digits only")]
    AppId,
    #[error("the installation ID must be digits only")]
    InstallationId,
    #[error("the private key is longer than 16 KiB")]
    TooLong,
    #[error("the private key is not an RSA private key in PEM form")]
    NotRsaPem,
    #[error("the private key is shorter than 2048 bits")]
    TooShort,
}

impl KeyRefusal {
    /// The `GithubAppInput` field the operator must fix.
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::AppId => "appId",
            Self::InstallationId => "installationId",
            Self::TooLong | Self::NotRsaPem | Self::TooShort => "privateKey",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the App token could not be signed")]
pub struct SignFailed;

/// Signs as the App before an installation is chosen (PRJ-039).
pub struct AppSigner {
    pub app_id: u64,
    pair: RsaKeyPair,
}

pub struct AppKey {
    signer: AppSigner,
    installation_id: String,
}

/// The only App ID or installation ID that enters a path.
#[must_use]
pub fn digits(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_ID_DIGITS && text.bytes().all(|b| b.is_ascii_digit())
}

fn pair(pem: &[u8]) -> Result<RsaKeyPair, KeyRefusal> {
    if pem.len() > MAX_PEM_BYTES {
        return Err(KeyRefusal::TooLong);
    }
    let der =
        Zeroizing::new(PrivateKeyDer::from_pem_slice(pem).map_err(|_| KeyRefusal::NotRsaPem)?);
    let pair = match &*der {
        PrivateKeyDer::Pkcs1(key) => RsaKeyPair::from_der(key.secret_pkcs1_der()),
        PrivateKeyDer::Pkcs8(key) => RsaKeyPair::from_pkcs8(key.secret_pkcs8_der()),
        _ => return Err(KeyRefusal::NotRsaPem),
    }
    // ring refuses a key under 2048 bits itself; its reason is a fixed word.
    .map_err(|rejected| {
        if rejected.to_string() == "TooSmall" {
            KeyRefusal::TooShort
        } else {
            KeyRefusal::NotRsaPem
        }
    })?;
    if pair.public().modulus_len() * 8 < MIN_KEY_BITS {
        return Err(KeyRefusal::TooShort);
    }
    Ok(pair)
}

impl AppSigner {
    /// # Errors
    /// [`KeyRefusal::AppId`] for App ID 0; the key's refusal otherwise.
    pub fn read(app_id: u64, pem: &[u8]) -> Result<Self, KeyRefusal> {
        if app_id == 0 {
            return Err(KeyRefusal::AppId);
        }
        Ok(Self {
            app_id,
            pair: pair(pem)?,
        })
    }

    /// # Errors
    /// [`KeyRefusal::InstallationId`] when it is not 1 to 20 digits.
    pub fn bind(self, installation_id: &str) -> Result<AppKey, KeyRefusal> {
        if !digits(installation_id) {
            return Err(KeyRefusal::InstallationId);
        }
        Ok(AppKey {
            signer: self,
            installation_id: installation_id.to_owned(),
        })
    }

    /// # Errors
    /// [`SignFailed`] when the random source fails.
    pub fn token(
        &self,
        now: DateTime<Utc>,
        random: &(dyn SecureRandom + Sync),
    ) -> Result<Zeroizing<String>, SignFailed> {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
        let claims = format!(
            r#"{{"iat":{},"exp":{},"iss":{}}}"#,
            (now - ISSUED_BEFORE).timestamp(),
            (now + VALID_FOR).timestamp(),
            self.app_id
        );
        // Room for an 8192-bit signature, so the buffer never grows and leaves no copy.
        let mut jwt = Zeroizing::new(String::with_capacity(2048));
        jwt.push_str(&header);
        jwt.push('.');
        jwt.push_str(&URL_SAFE_NO_PAD.encode(claims));
        let mut signature = vec![0; self.pair.public().modulus_len()];
        self.pair
            .sign(&RSA_PKCS1_SHA256, random, jwt.as_bytes(), &mut signature)
            .map_err(|_| SignFailed)?;
        jwt.push('.');
        jwt.push_str(&URL_SAFE_NO_PAD.encode(signature));
        Ok(jwt)
    }

    #[cfg(test)]
    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        self.pair.public().as_ref()
    }
}

impl AppKey {
    /// # Errors
    /// [`KeyRefusal`] for the first of App ID, installation ID and key that is not right.
    pub fn read(app_id: &str, installation_id: &str, pem: &[u8]) -> Result<Self, KeyRefusal> {
        let app_id = Some(app_id)
            .filter(|id| digits(id))
            .and_then(|id| id.parse::<u64>().ok())
            .filter(|id| *id > 0)
            .ok_or(KeyRefusal::AppId)?;
        if !digits(installation_id) {
            return Err(KeyRefusal::InstallationId);
        }
        AppSigner::read(app_id, pem)?.bind(installation_id)
    }

    #[must_use]
    pub fn instance_key(&self) -> String {
        format!("{}/{}", self.signer.app_id, self.installation_id)
    }

    #[must_use]
    pub fn installation_id(&self) -> &str {
        &self.installation_id
    }

    /// # Errors
    /// [`SignFailed`] when the random source fails.
    pub fn token(
        &self,
        now: DateTime<Utc>,
        random: &(dyn SecureRandom + Sync),
    ) -> Result<Zeroizing<String>, SignFailed> {
        self.signer.token(now, random)
    }

    #[cfg(test)]
    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        self.signer.public_key()
    }
}

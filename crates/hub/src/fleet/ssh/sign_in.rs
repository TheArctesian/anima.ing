//! OD-9, D-5. Nothing here logs; no error holds the device's text or a secret.

use std::sync::Arc;

use russh::{
    MethodKind,
    client::{AuthResult, Handle, KeyboardInteractiveAuthResponse},
    keys::{PrivateKeyWithHashAlg, decode_secret_key},
};

use super::{Observer, SshError};
use crate::fleet::credentials::{Secret, SshSecret};

/// Bounds the rounds answered, so a device cannot keep the hub answering prompts (D-5).
const PROMPT_ROUNDS: usize = 4;
/// A round with more is a refusal, so a device cannot have the password copied without end (D-5).
const PROMPTS_PER_ROUND: usize = 4;

/// russh copies what it sends into buffers of its own, which this cannot wipe.
pub(crate) async fn authenticate(
    handle: &mut Handle<Observer>,
    username: &str,
    secret: &SshSecret,
) -> Result<(), SshError> {
    let result = match secret {
        SshSecret::Password(password) => {
            let password =
                std::str::from_utf8(password.expose()).map_err(|_| SshError::AuthFailed)?;
            sign_in_with_password(handle, username, password).await?
        }
        SshSecret::PrivateKey { key, passphrase } => {
            let key = private_key(key.expose(), passphrase.as_ref().map(Secret::expose))?;
            let hash = if key.algorithm().is_rsa() {
                handle
                    .best_supported_rsa_hash()
                    .await
                    .map_err(|_| SshError::Protocol)?
                    .flatten()
            } else {
                None
            };
            let key = PrivateKeyWithHashAlg::new(Arc::new(key), hash);
            handle
                .authenticate_publickey(username, key)
                .await
                .map_err(|_| SshError::Protocol)?
        }
    };
    match result {
        AuthResult::Success => Ok(()),
        AuthResult::Failure { .. } => Err(SshError::AuthFailed),
    }
}

async fn sign_in_with_password(
    handle: &mut Handle<Observer>,
    username: &str,
    password: &str,
) -> Result<AuthResult, SshError> {
    let result = handle
        .authenticate_password(username, password)
        .await
        .map_err(|_| SshError::Protocol)?;
    let AuthResult::Failure {
        remaining_methods, ..
    } = &result
    else {
        return Ok(result);
    };
    let prompts_only = remaining_methods.contains(&MethodKind::KeyboardInteractive)
        && !remaining_methods.contains(&MethodKind::Password);
    if !prompts_only {
        return Ok(result);
    }
    answer_prompts(handle, username, password).await
}

/// The prompts are the device's text: never read, logged or kept.
async fn answer_prompts(
    handle: &mut Handle<Observer>,
    username: &str,
    password: &str,
) -> Result<AuthResult, SshError> {
    let mut reply = handle
        .authenticate_keyboard_interactive_start(username, None)
        .await
        .map_err(|_| SshError::Protocol)?;
    for _ in 0..PROMPT_ROUNDS {
        let KeyboardInteractiveAuthResponse::InfoRequest { prompts, .. } = reply else {
            break;
        };
        let prompts = prompts.len();
        if prompts > PROMPTS_PER_ROUND {
            return Err(SshError::AuthFailed);
        }
        let answers = vec![password.to_owned(); prompts];
        reply = handle
            .authenticate_keyboard_interactive_respond(answers)
            .await
            .map_err(|_| SshError::Protocol)?;
    }
    // The reply to the last allowed answer counts; another round of prompts does not.
    match reply {
        KeyboardInteractiveAuthResponse::Success => Ok(AuthResult::Success),
        _ => Err(SshError::AuthFailed),
    }
}

fn private_key(key: &[u8], passphrase: Option<&[u8]>) -> Result<russh::keys::PrivateKey, SshError> {
    let key = std::str::from_utf8(key).map_err(|_| SshError::UnreadableKey)?;
    let passphrase = passphrase
        .map(std::str::from_utf8)
        .transpose()
        .map_err(|_| SshError::UnreadableKey)?;
    decode_secret_key(key, passphrase).map_err(|_| SshError::UnreadableKey)
}

//! API-key input for `anima auth login --with-token` (CMD-011). Browser approval lives in `device`.

use std::path::Path;

use zeroize::Zeroizing;

use super::documents::{Me, me};
use crate::failure::Failure;
use crate::hub::{Hub, HubError, Origin, Token};
use crate::input;
use crate::prompt::Prompter;

/// The longest API key read from standard input.
const MAX_KEY: usize = 4096;

/// A token the hub accepted, and the user it belongs to.
pub struct SignedIn {
    pub token: Token,
    pub username: String,
}

/// Reads a token from standard input (echo off at a terminal) and checks it with `me`.
///
/// # Errors
/// [`Failure::Input`] for an unreadable or empty input; the hub's refusal of the token.
pub async fn with_token(
    origin: Origin,
    prompter: Prompter,
    stdin_terminal: bool,
) -> Result<SignedIn, Failure> {
    let text = if stdin_terminal {
        prompter.ask_secret("API key", "the key on standard input")?
    } else {
        read_stdin(prompter)?
    };
    let token = Token::new(text).ok_or_else(|| Failure::Input {
        what: "the token".to_owned(),
        reason: "standard input held no token".to_owned(),
    })?;
    let hub = Hub::new(origin, Some(token))?;
    let answer = hub.run::<Me>(me::Variables).await?;
    let token = hub.into_token().ok_or(HubError::Setup)?;
    Ok(SignedIn {
        token,
        username: answer.me.username,
    })
}

/// The token on standard input, as text; never echoed or kept (CMD-014).
pub(super) fn read_stdin(prompter: Prompter) -> Result<Zeroizing<String>, Failure> {
    let bytes = input::read(prompter, Path::new("-"), "the token", MAX_KEY)?;
    std::str::from_utf8(&bytes)
        .map(|text| Zeroizing::new(text.to_owned()))
        .map_err(|_| Failure::Input {
            what: "the token".to_owned(),
            reason: "standard input is not text".to_owned(),
        })
}

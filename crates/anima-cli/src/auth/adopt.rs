//! `anima auth adopt`: the token that the Anima app gives the `anima` program it carries (R-383,
//! SGN-NEW-adopt). The token comes on standard input only, never from a terminal, an argument
//! or an environment variable (CMD-014, LOG-007). It is checked with `me`, kept as `anima auth
//! login` keeps a token (CMD-012), and its hub becomes the current context, so that a hub that
//! the app reaches through the link (`http://127.0.0.1:<port>`) is the hub of the command line.

use super::documents::{Me, me};
use super::login::read_stdin;
use crate::contexts::config::{Config, Context};
use crate::failure::Failure;
use crate::hub::{Hub, HubError, Token, TokenKind};
use crate::invocation::Invocation;

/// The context that holds the hub of the app.
pub const APP_CONTEXT: &str = "app";

/// The user that the adopted token signs in as.
pub struct Adopted {
    pub username: String,
}

/// Reads the app's token from standard input, checks it with the hub of this run, keeps it
/// and makes that hub the current context [`APP_CONTEXT`].
///
/// # Errors
/// [`Failure::Usage`] when standard input is a terminal or holds an API key;
/// [`Failure::Input`] for no token; the hub's refusal of the token; a store failure.
pub async fn adopt(invocation: &Invocation) -> Result<Adopted, Failure> {
    if invocation.stdin_terminal() {
        return Err(not_from_a_person());
    }
    let origin = invocation.origin()?;
    let text = read_stdin(invocation.prompter)?;
    let token = Token::new(text).ok_or_else(|| Failure::Input {
        what: "the token".to_owned(),
        reason: "standard input held no token".to_owned(),
    })?;
    if token.kind() != TokenKind::Login {
        return Err(not_from_a_person());
    }
    let hub = Hub::new(origin.clone(), Some(token))?;
    let answer = hub.run::<Me>(me::Variables).await?;
    let token = hub.into_token().ok_or(HubError::Setup)?;
    invocation.token_store().write(&origin, &token)?;
    let dir = invocation.config_dir()?;
    let mut config = Config::load(dir)?;
    make_current(&mut config, &origin.as_text());
    config.save(dir)?;
    Ok(Adopted {
        username: answer.me.username,
    })
}

/// Points the context [`APP_CONTEXT`] at `hub` and makes it current.
fn make_current(config: &mut Config, hub: &str) {
    config.contexts.insert(
        APP_CONTEXT.to_owned(),
        Context {
            hub: hub.to_owned(),
        },
    );
    config.current = Some(APP_CONTEXT.to_owned());
}

fn not_from_a_person() -> Failure {
    Failure::Usage {
        message: "anima auth adopt takes the token that the Anima.ing application gives on \
                  standard input. To sign in yourself, run anima auth login."
            .to_owned(),
        hint: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sgn_adopt_the_hub_of_the_app_becomes_the_current_context() {
        let mut config = Config {
            current: Some("home".into()),
            ..Config::default()
        };
        config.contexts.insert(
            "home".into(),
            Context {
                hub: "https://home.example".into(),
            },
        );
        make_current(&mut config, "http://127.0.0.1:41641");
        assert_eq!(config.current.as_deref(), Some(APP_CONTEXT));
        assert_eq!(config.contexts[APP_CONTEXT].hub, "http://127.0.0.1:41641");
        assert_eq!(config.contexts["home"].hub, "https://home.example");
        make_current(&mut config, "http://127.0.0.1:41642");
        assert_eq!(config.contexts[APP_CONTEXT].hub, "http://127.0.0.1:41642");
    }
}

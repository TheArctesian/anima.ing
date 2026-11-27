//! `anima auth`: sign in, sign out and show who is signed in (CMD-010..014), and take the
//! token that the Anima app gives (R-383). No command here prints a token.

pub mod adopt;
mod adoption;
mod device;
pub mod documents;
mod hosted;
mod lock;
pub mod login;
mod remote_link;
mod sessions;
pub mod token_file;
pub mod token_store;

use serde::Serialize;

use crate::failure::Failure;
use crate::hub::{Code, HubError, Origin, Token, TokenKind};
use crate::invocation::{Invocation, TokenSource};
use crate::output::{Cell, Table};
use documents::{Logout, Me, logout, me};

#[derive(Debug, clap::Subcommand)]
pub enum AuthCommand {
    /// Sign in to the hub and keep the token in the key store.
    Login {
        /// Read an API key (anima_...) from standard input instead of browser approval.
        #[arg(long)]
        with_token: bool,
        /// Approve this remote computer using your Anima account in a browser.
        #[arg(long, conflicts_with_all = ["with_token", "hub", "context"])]
        hosted: bool,
        /// Open the approval page without asking, when input is interactive.
        #[arg(long, conflicts_with = "with_token")]
        web: bool,
        /// The name shown when approving this computer.
        #[arg(long, conflicts_with = "with_token")]
        computer: Option<String>,
    },
    /// Take the token that the Anima.ing application gives on standard input, for the hub of --hub, and
    /// make that hub the current context. The app runs it; a person runs `anima auth login`.
    Adopt {
        /// `-`: the token is on standard input, and only there.
        #[arg(value_parser = ["-"])]
        input: String,
        /// The app provides its session; issue and store a linked CLI credential atomically.
        #[arg(long)]
        app_session: bool,
    },
    /// End the stored login token at the hub and delete it here.
    Logout {
        /// Remove the local credential even if the hub is unreachable (used by the app).
        #[arg(long)]
        forget_on_error: bool,
    },
    /// Show the hub, the user and the kind of token.
    Status,
    /// List command-line sessions. Remote sessions have no automatic expiry.
    Sessions {
        #[arg(long, default_value_t = 0)]
        offset: i64,
    },
    /// Revoke a session listed by auth sessions.
    Revoke { id: String },
}

/// What `auth status` prints; `--json` keeps these names.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    hub: String,
    operator: me::MeMe,
    token_kind: TokenKind,
    token_source: TokenSource,
}

impl AuthCommand {
    /// Runs the command.
    ///
    /// # Errors
    /// [`Failure`] for a refusal, a missing value or a store failure.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        match self {
            Self::Login {
                with_token,
                hosted,
                web,
                computer,
            } => sign_in(invocation, (with_token, hosted, web), computer).await,
            Self::Adopt {
                input: _,
                app_session,
            } => take_from_app(invocation, app_session).await,
            Self::Logout { forget_on_error } => sign_out(invocation, forget_on_error).await,
            Self::Status => status(invocation).await,
            Self::Sessions { offset } => sessions::list(invocation, offset).await,
            Self::Revoke { id } => sessions::revoke(invocation, id).await,
        }
    }
}

enum Destination {
    Hosted,
    Hub(Origin),
}

fn destination(
    invocation: &Invocation,
    hosted: bool,
    with_token: bool,
) -> Result<Destination, Failure> {
    if hosted {
        return Ok(Destination::Hosted);
    }
    if with_token || invocation.explicit_hub() {
        return invocation.origin().map(Destination::Hub);
    }
    let choice = invocation.prompter.ask(
        "Sign in: 1) Anima account  2) A hub of your own [1]",
        "--hosted or --hub",
    )?;
    match choice.as_str() {
        "" | "1" => Ok(Destination::Hosted),
        "2" => {
            let address = invocation.prompter.ask("Hub address", "--hub")?;
            let origin = Origin::parse(&address).map_err(|error| Failure::Usage {
                message: error.to_string(),
                hint: None,
            })?;
            Ok(Destination::Hub(origin))
        }
        _ => Err(Failure::Usage {
            message: "Choose 1 or 2.".to_owned(),
            hint: None,
        }),
    }
}

async fn sign_in(
    invocation: &Invocation,
    (with_token, hosted, web): (bool, bool, bool),
    computer: Option<String>,
) -> Result<(), Failure> {
    let destination = destination(invocation, hosted, with_token)?;
    let computer = computer.unwrap_or_else(this_computer);
    let prompter = invocation.prompter;
    let (origin, signed_in) = match destination {
        Destination::Hosted => hosted::sign_in(invocation, web, &computer).await?,
        Destination::Hub(origin) => {
            let signed_in = if with_token {
                login::with_token(origin.clone(), prompter, invocation.stdin_terminal()).await?
            } else {
                device::sign_in(invocation, origin.clone(), web, computer).await?
            };
            (origin, signed_in)
        }
    };
    let _lock = lock::acquire(invocation.config_dir()?).await?;
    invocation.token_store().write(&origin, &signed_in.token)?;
    if !with_token {
        let mut config = invocation.config()?;
        let name = config
            .contexts
            .iter()
            .find(|(_, context)| context.hub == origin.as_text())
            .map_or_else(|| origin.as_text(), |(name, _)| name.clone());
        config.contexts.insert(
            name.clone(),
            crate::contexts::config::Context {
                hub: origin.as_text(),
            },
        );
        config.current = Some(name);
        config.save(invocation.config_dir()?)?;
    }
    let kind = signed_in.token.kind().describe();
    invocation.output.note(&format!(
        "Signed in to {origin} as {} with {kind}.",
        signed_in.username
    ));
    if invocation.has_token_variable() {
        invocation
            .output
            .note("ANIMA_TOKEN is set, and commands use it before the stored token.");
    }
    Ok(())
}

async fn take_from_app(invocation: &Invocation, app_session: bool) -> Result<(), Failure> {
    let adopted = adopt::adopt_session(invocation, app_session).await?;
    let origin = invocation.origin()?;
    invocation.output.note(&format!(
        "Signed in to {origin} as {} with the sign-in of the Anima.ing application. The current context is {}.",
        adopted.username,
        adopt::APP_CONTEXT
    ));
    Ok(())
}

async fn sign_out(invocation: &Invocation, forget_on_error: bool) -> Result<(), Failure> {
    let _lock = lock::acquire(invocation.config_dir()?).await?;
    let origin = invocation.origin()?;
    let store = invocation.token_store();
    let Some(token) = store.read(&origin)? else {
        store.delete(&origin)?;
        invocation
            .output
            .note(&format!("Local sign-in is cleared for {origin}."));
        return Ok(());
    };
    let kind = token.kind();
    if kind == TokenKind::Login {
        let hub = crate::hub::Hub::new(origin.clone(), Some(token))?;
        match hub.run::<Logout>(logout::Variables).await {
            // An expired or revoked token is already signed out.
            Ok(_)
            | Err(HubError::Refused {
                code: Code::Unauthenticated,
                ..
            }) => {}
            Err(error) => {
                if forget_on_error {
                    store.delete(&origin)?;
                }
                return Err(error.into());
            }
        }
    }
    store.delete(&origin)?;
    invocation.output.note(&format!("Signed out of {origin}."));
    if kind == TokenKind::ApiKey {
        invocation
            .output
            .note("The API key itself still works until it expires or is revoked.");
    }
    Ok(())
}

async fn status(invocation: &Invocation) -> Result<(), Failure> {
    let (hub, source) = invocation.signed_in()?;
    let answer = hub.run::<Me>(me::Variables).await?;
    let status = Status {
        hub: hub.origin().as_text(),
        operator: answer.me,
        token_kind: hub.token().map_or(TokenKind::Login, Token::kind),
        token_source: source,
    };
    invocation.output.data(&status, |status| {
        let from = match status.token_source {
            TokenSource::Environment => "from ANIMA_TOKEN",
            TokenSource::Store => "from the key store",
        };
        let mut table = Table::record();
        table.pair("Hub", status.hub.as_str());
        table.pair("User", status.operator.username.as_str());
        table.pair("Token", format!("{}, {from}", status.token_kind.describe()));
        table.pair(
            "Authenticator",
            Cell::new(if status.operator.totp_enabled {
                "on"
            } else {
                "off"
            }),
        );
        table
    })
}

/// The default sign-in label must pass the hub's computer-name validator (CMD-010).
fn this_computer() -> String {
    let host = whoami::hostname().unwrap_or_default();
    computer_name(&host)
}

fn computer_name(host: &str) -> String {
    let clean: String = host.chars().filter(|c| !c.is_control()).collect();
    let name: String = clean.trim().chars().take(64).collect();
    let name = name.trim_end();
    if name.is_empty() {
        "This computer".to_owned()
    } else {
        name.to_owned()
    }
}

#[cfg(test)]
mod computer_tests {
    use super::computer_name;

    #[test]
    fn cmd_010_the_default_computer_name_is_one_the_hub_takes() {
        assert_eq!(computer_name("mac-1.local\0"), "mac-1.local");
        assert_eq!(computer_name(" \t\0"), "This computer");
        assert_eq!(computer_name(&"a".repeat(100)).chars().count(), 64);
        assert_eq!(computer_name(&"é".repeat(65)), "é".repeat(64));
        assert_eq!(
            computer_name(&format!("{} b", "z".repeat(63))),
            "z".repeat(63)
        );
        assert_eq!(computer_name("pop-os"), "pop-os");
    }
}

use std::{
    process::Command,
    time::{Duration, Instant},
};

use graphql_client::GraphQLQuery;
use zeroize::Zeroizing;

use super::{
    documents::{Me, me},
    login::SignedIn,
};
use crate::hub::scalars::DateTime;
use crate::{
    failure::Failure,
    hub::{Hub, HubError, Origin, Token},
    invocation::Invocation,
};

#[derive(GraphQLQuery)]
#[graphql(schema_path = "schema.graphql", query_path = "src/auth/device.graphql")]
struct StartCommandLineSignIn;

#[derive(GraphQLQuery)]
#[graphql(schema_path = "schema.graphql", query_path = "src/auth/device.graphql")]
struct PollCommandLineSignIn;

pub(super) async fn sign_in(
    invocation: &Invocation,
    origin: Origin,
    web: bool,
    computer: String,
) -> Result<SignedIn, Failure> {
    let hub = Hub::new(origin.clone(), None)?;
    let start = hub
        .run::<StartCommandLineSignIn>(start_command_line_sign_in::Variables { computer })
        .await?
        .start_command_line_sign_in;
    let code = Zeroizing::new(start.device_code);
    let uri = approved_page(&origin, &start.verification_uri)?;
    show_code(invocation, web, &start.user_code, &uri)?;
    wait(invocation, hub, code, start.interval).await
}

pub(super) fn show_code(
    invocation: &Invocation,
    web: bool,
    code: &str,
    uri: &str,
) -> Result<(), Failure> {
    invocation.output.message(&format!(
        "Your one-time code: {code}\nEnter it at {uri}. It expires in ten minutes."
    ));
    if invocation.prompter.interactive()
        && (web
            || invocation
                .prompter
                .ask("Press Enter to open the browser", "--web")?
                .is_empty())
        && !open_browser(uri)
    {
        invocation
            .output
            .message("Open the address above in a browser on this or another computer.");
    }
    Ok(())
}

async fn wait(
    invocation: &Invocation,
    hub: Hub,
    code: Zeroizing<String>,
    initial_interval: i64,
) -> Result<SignedIn, Failure> {
    use poll_command_line_sign_in::CommandLineSignInStatus;
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut interval = interval(initial_interval)?;
    loop {
        if Instant::now() + interval >= deadline {
            return Err(ended(
                "The sign-in code expired. Run anima auth login again.",
            ));
        }
        tokio::time::sleep(interval).await;
        let mut request =
            PollCommandLineSignIn::build_query(poll_command_line_sign_in::Variables {
                device_code: code.to_string(),
            });
        let result = hub
            .send::<_, poll_command_line_sign_in::ResponseData>(&request)
            .await;
        zeroize::Zeroize::zeroize(&mut request.variables.device_code);
        let answer = result?.poll_command_line_sign_in;
        interval = self::interval(answer.interval)?.max(interval);
        match answer.status {
            CommandLineSignInStatus::WAITING | CommandLineSignInStatus::SLOW_DOWN => {}
            CommandLineSignInStatus::DENIED => return Err(ended("The sign-in was denied.")),
            CommandLineSignInStatus::EXPIRED => {
                return Err(ended(
                    "The sign-in code expired. Run anima auth login again.",
                ));
            }
            CommandLineSignInStatus::APPROVED => {
                let text = answer.token.ok_or(HubError::Malformed { status: 200 })?;
                let token =
                    Token::new(Zeroizing::new(text)).ok_or(HubError::Malformed { status: 200 })?;
                let signed_in = Hub::new(hub.origin().clone(), Some(token))?;
                let account = signed_in.run::<Me>(me::Variables).await?.me;
                invocation.output.note(
                    "Browser approval received. This remote session stays signed in until revoked.",
                );
                return Ok(SignedIn {
                    token: signed_in.into_token().ok_or(HubError::Setup)?,
                    username: account.username,
                });
            }
            CommandLineSignInStatus::Other(_) => {
                return Err(HubError::Malformed { status: 200 }.into());
            }
        }
    }
}

fn interval(seconds: i64) -> Result<Duration, Failure> {
    let seconds = u64::try_from(seconds)
        .ok()
        .filter(|value| (5..=600).contains(value))
        .ok_or(HubError::Malformed { status: 200 })?;
    Ok(Duration::from_secs(seconds))
}

fn approved_page(origin: &Origin, supplied: &str) -> Result<String, Failure> {
    let url = reqwest::Url::parse(supplied).map_err(|_| HubError::Malformed { status: 200 })?;
    if url.origin().ascii_serialization() != origin.as_text()
        || url.path() != "/device"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(HubError::Malformed { status: 200 }.into());
    }
    Ok(url.to_string())
}

fn open_browser(uri: &str) -> bool {
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(uri).status();
    #[cfg(target_os = "windows")]
    let result = Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", uri])
        .status();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = Command::new("xdg-open").arg(uri).status();
    result.is_ok_and(|status| status.success())
}

pub(super) fn ended(message: &str) -> Failure {
    Failure::Usage {
        message: message.to_owned(),
        hint: None,
    }
}

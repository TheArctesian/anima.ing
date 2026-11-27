//! Interactive provider challenges never enter durable command receipts.
use super::{
    args::Provider,
    documents::{
        ManagedProviderLogin, ManagedProviderReply, managed_provider_login, managed_provider_reply,
    },
};
use crate::{
    devices::get::resolve,
    failure::Failure,
    hub::{Hub, subscription::Subscription},
    invocation::Invocation,
    prompt::Prompter,
    raw_mode::RawMode,
};
use graphql_client::GraphQLQuery;
use serde_json::Value;
use std::{
    io::{IsTerminal, Write},
    time::Duration,
};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

type Events = Subscription<managed_provider_login::ResponseData>;

pub(super) async fn run(
    invocation: &Invocation,
    target: (String, Uuid, u64),
    provider: Provider,
) -> Result<(), Failure> {
    if invocation.output.json()
        || !std::io::stdout().is_terminal()
        || !invocation.prompter.interactive()
    {
        return Err(Failure::Usage {
            message: "Provider sign-in requires an interactive terminal.".into(),
            hint: None,
        });
    }
    let hub = invocation.hub()?;
    let device = resolve(&hub, &target.0).await?;
    let provider = match provider {
        Provider::Codex => "codex",
        Provider::Claude => "claude",
    };
    let mut events = Subscription::open::<ManagedProviderLogin>(
        &hub,
        managed_provider_login::Variables {
            device,
            workspace: target.1.to_string(),
            generation: target.2.to_string(),
            provider: Some(provider.into()),
        },
    )
    .await?;
    tokio::time::timeout(
        Duration::from_secs(600),
        interact(&hub, &mut events, invocation.prompter),
    )
    .await
    .map_err(|_| ended("Provider sign-in expired."))?
}

async fn interact(hub: &Hub, events: &mut Events, prompter: Prompter) -> Result<(), Failure> {
    let mut challenged = false;
    while let Some(answer) = events.next().await {
        let event = answer?.managed_code_provider_login;
        match event["state"].as_str() {
            Some("challenge") if !challenged => {
                let code = event["code"]
                    .as_str()
                    .filter(|code| {
                        !code.is_empty()
                            && code.len() <= 64
                            && code
                                .bytes()
                                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                    })
                    .ok_or_else(|| ended("Invalid provider challenge."))?;
                if event["verification_url"] != "https://auth.openai.com/codex/device" {
                    return Err(ended("Invalid provider challenge."));
                }
                show(&format!(
                    "Open https://auth.openai.com/codex/device and enter {code}"
                ))?;
                challenged = true;
            }
            Some("paste-challenge") if !challenged => {
                paste(hub, events, prompter, &event).await?;
                challenged = true;
            }
            Some("succeeded") if challenged => {
                show("Provider sign-in completed.")?;
                return Ok(());
            }
            Some("expired") => return Err(ended("Provider sign-in expired.")),
            Some("cancelled") => return Err(Failure::Cancelled),
            _ => return Err(ended("Provider sign-in was not confirmed.")),
        }
    }
    Err(ended("Provider sign-in ended without confirmation."))
}

async fn paste(
    hub: &Hub,
    events: &mut Events,
    prompter: Prompter,
    event: &Value,
) -> Result<(), Failure> {
    let url = event["verification_url"]
        .as_str()
        .filter(|raw| valid_url(raw))
        .ok_or_else(|| ended("Invalid provider challenge."))?;
    let session = event["session"]
        .as_str()
        .and_then(|id| id.parse::<Uuid>().ok())
        .filter(|id| !id.is_nil())
        .ok_or_else(|| ended("Invalid provider session."))?;
    show(&format!("Open {url} in your browser."))?;
    let mut terminal = prompter.hide("Returned code", "interactive sign-in")?;
    let _restore = RestoreTerminal;
    let (send, mut receive) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = send.send(terminal.ask_secret("Returned code"));
    });
    let code = tokio::select! {
        answer = &mut receive => answer.map_err(|_| ended("The sign-in prompt ended."))??,
        event = events.next() => {
            if let Some(answer) = event { let _ = answer?; }
            return Err(ended("Provider sign-in ended while waiting for the code."));
        }
    };
    if !agent_threads::managed::provider_login::valid_code(&code) {
        return Err(ended("Invalid returned code."));
    }
    submit(hub, session, code).await
}

async fn submit(hub: &Hub, session: Uuid, code: Zeroizing<String>) -> Result<(), Failure> {
    let mut body =
        <ManagedProviderReply as GraphQLQuery>::build_query(managed_provider_reply::Variables {
            session: session.to_string(),
            code: code.to_string(),
        });
    let answer = hub
        .send::<_, managed_provider_reply::ResponseData>(&body)
        .await;
    body.variables.code.zeroize();
    if !answer?.reply_managed_code_provider_login {
        return Err(ended("Provider reply was not accepted."));
    }
    Ok(())
}

fn valid_url(raw: &str) -> bool {
    if raw.len() > 2048
        || raw
            .bytes()
            .any(|byte| !byte.is_ascii() || byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(raw) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str() == Some("claude.com")
        && url.path() == "/cai/oauth/authorize"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.fragment().is_none()
}
fn show(text: &str) -> Result<(), Failure> {
    writeln!(std::io::stdout(), "{text}").map_err(|_| ended("The sign-in terminal closed."))
}
fn ended(message: &str) -> Failure {
    Failure::NotDelivered {
        message: message.into(),
    }
}
struct RestoreTerminal;
impl Drop for RestoreTerminal {
    fn drop(&mut self) {
        RawMode::restore();
    }
}

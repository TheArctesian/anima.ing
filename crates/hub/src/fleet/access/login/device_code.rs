#![expect(
    clippy::unused_async_trait_impl,
    reason = "async-graphql generates the enum OutputType async trait implementation"
)]
mod approval;

use std::sync::Arc;

use async_graphql::{Context, Enum, SimpleObject};
use chrono::{DateTime, TimeDelta, Utc};
use sha2::{Digest as _, Sha256};
use sqlx::PgPool;
use zeroize::{Zeroize as _, Zeroizing};

use super::{command_line, random};
use crate::{
    clock::Clock,
    config::Config,
    fleet::access::{
        ClientAddr, Error,
        store::sign_in as store,
        tokens::{self, Token},
    },
};

pub use approval::CommandLineSignInDetails;
pub(in crate::fleet::access) use approval::{approve, describe};

#[derive(SimpleObject)]
pub struct CommandLineSignIn {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_at: DateTime<Utc>,
    interval: i32,
}

impl Drop for CommandLineSignIn {
    fn drop(&mut self) {
        self.device_code.zeroize();
        self.user_code.zeroize();
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Enum)]
pub enum CommandLineSignInStatus {
    Waiting,
    SlowDown,
    Denied,
    Expired,
    Approved,
}

#[derive(SimpleObject)]
pub struct CommandLineSignInState {
    status: CommandLineSignInStatus,
    token: Option<String>,
    interval: i32,
}

impl Drop for CommandLineSignInState {
    fn drop(&mut self) {
        self.token.zeroize();
    }
}

impl CommandLineSignInState {
    fn pending(status: CommandLineSignInStatus, interval: i32) -> Self {
        Self {
            status,
            token: None,
            interval,
        }
    }
}

struct Flow<'a> {
    db: &'a PgPool,
    now: DateTime<Utc>,
    client: ClientAddr,
    config: &'a Config,
}

impl<'a> Flow<'a> {
    fn of(ctx: &'a Context<'_>) -> Result<Self, Error> {
        Ok(Self {
            db: ctx.data_opt::<PgPool>().ok_or(Error::Unavailable)?,
            now: ctx
                .data_opt::<Arc<dyn Clock>>()
                .ok_or(Error::Unavailable)?
                .now(),
            client: *ctx.data_opt::<ClientAddr>().ok_or(Error::Unavailable)?,
            config: ctx.data_opt::<Arc<Config>>().ok_or(Error::Unavailable)?,
        })
    }
}

pub(in crate::fleet::access) async fn start(
    ctx: &Context<'_>,
    computer: String,
) -> async_graphql::Result<CommandLineSignIn> {
    ctx.insert_http_header("cache-control", "no-store");
    let flow = Flow::of(ctx).map_err(Error::into_graphql)?;
    start_at(flow, computer).await.map_err(Error::into_graphql)
}

async fn start_at(flow: Flow<'_>, computer: String) -> Result<CommandLineSignIn, Error> {
    if flow.config.hosted_cloud_key_file.is_some() {
        return Err(Error::Hosted);
    }
    let computer = command_line::computer_name(&computer)?;
    let device = Token::from_random(random()?);
    let user = user_code()?;
    let hash = user_hash(&user);
    let client = flow.client.0.to_string();
    let request = store::NewSignIn {
        device: device.hash(),
        user: &hash,
        computer,
        client: &client,
    };
    let mut tx = flow.db.begin().await?;
    if !store::start_sign_in(&mut tx, &request, flow.now).await? {
        return Err(Error::Validation {
            field: "computer",
            message: "Too many sign-in attempts. Try again later.",
        });
    }
    tx.commit().await?;
    Ok(CommandLineSignIn {
        device_code: device.expose().to_owned(),
        user_code: format!("{}-{}", &user[..4], &user[4..]),
        verification_uri: format!("{}/device", flow.config.public_origin.trim_end_matches('/')),
        expires_at: flow.now + TimeDelta::minutes(10),
        interval: 5,
    })
}

fn user_code() -> Result<Zeroizing<String>, Error> {
    const LETTERS: &[u8; 20] = b"BCDFGHJKLMNPQRSTVWXZ";
    let mut code = Zeroizing::new(String::with_capacity(8));
    while code.len() < 8 {
        let [byte] = random::<1>()?;
        if byte < 240 {
            code.push(char::from(LETTERS[usize::from(byte % 20)]));
        }
    }
    Ok(code)
}

fn user_hash(code: &str) -> [u8; 32] {
    let normalized = Zeroizing::new(code.trim().replace('-', "").to_ascii_uppercase());
    Sha256::digest(normalized.as_bytes()).into()
}

pub(in crate::fleet::access) async fn poll(
    ctx: &Context<'_>,
    device_code: String,
) -> async_graphql::Result<CommandLineSignInState> {
    let code = Zeroizing::new(device_code);
    ctx.insert_http_header("cache-control", "no-store");
    let flow = Flow::of(ctx).map_err(Error::into_graphql)?;
    poll_at(flow, &code).await.map_err(Error::into_graphql)
}

async fn poll_at(flow: Flow<'_>, code: &str) -> Result<CommandLineSignInState, Error> {
    use CommandLineSignInStatus::{Denied, Expired, SlowDown, Waiting};
    let Some(hash) = tokens::hash_of(code) else {
        return Ok(CommandLineSignInState::pending(Expired, 5));
    };
    let mut tx = flow.db.begin().await?;
    let Some(pending) = store::sign_in_by_device(&mut tx, hash).await? else {
        return Ok(CommandLineSignInState::pending(Expired, 5));
    };
    if pending.expires_at <= flow.now || matches!(pending.state.as_str(), "expired" | "consumed") {
        return Ok(CommandLineSignInState::pending(
            Expired,
            pending.interval_seconds,
        ));
    }
    if pending.state == "denied" {
        return Ok(CommandLineSignInState::pending(
            Denied,
            pending.interval_seconds,
        ));
    }
    if flow.now < pending.next_poll_at {
        let interval = pending.interval_seconds.saturating_add(5);
        store::pace_sign_in(&mut tx, pending.id, interval, flow.now).await?;
        tx.commit().await?;
        return Ok(CommandLineSignInState::pending(SlowDown, interval));
    }
    if pending.state == "approved" {
        let token = Token::from_random(random()?);
        store::insert_remote_token(&mut tx, &pending, token.hash(), flow.now).await?;
        store::finish_sign_in(&mut tx, pending.id, "consumed", pending.operator_id).await?;
        tx.commit().await?;
        return Ok(CommandLineSignInState {
            status: CommandLineSignInStatus::Approved,
            token: Some(token.expose().to_owned()),
            interval: pending.interval_seconds,
        });
    }
    store::pace_sign_in(&mut tx, pending.id, pending.interval_seconds, flow.now).await?;
    tx.commit().await?;
    Ok(CommandLineSignInState::pending(
        Waiting,
        pending.interval_seconds,
    ))
}

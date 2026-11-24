//! The only code that talks to the hub (CMD-003). It follows no redirect, ignores proxy variables
//! and keeps no cookie. It logs nothing, and no error it returns holds the token or the body (CMD-014).

pub mod account;
pub mod answer;
pub mod attachments;
pub mod error;
pub mod origin;
pub mod scalars;
pub mod subscription;
pub mod token;
pub mod tunnel;
mod wire;

use std::time::Duration;

use bytes::Bytes;
use graphql_client::{GraphQLQuery, QueryBody};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use reqwest::{Client, redirect};
use serde::Serialize;
use serde::de::DeserializeOwned;
use zeroize::Zeroizing;

pub use error::{Code, FieldError, HubError};
pub use origin::Origin;
pub use token::{Token, TokenKind};

/// How long one request may take, connection and answer together.
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// A page of 200 devices, the hub's page limit, is well under it.
const MAX_ANSWER: usize = 32 * 1024 * 1024;

/// The header of the agent declaration and its one value (K7, LOG-016).
pub const ACTOR_HEADER: &str = "anima-actor";
pub const AGENT: &str = "agent";

/// `Debug` shows the origin and the token's kind only.
#[derive(Debug)]
pub struct Hub {
    origin: Origin,
    token: Option<Token>,
    http: Client,
    declares_agent: bool,
}

impl Hub {
    /// Only `login` works without a token.
    ///
    /// # Errors
    /// [`HubError::Setup`] when the TLS roots cannot be loaded.
    pub fn new(origin: Origin, token: Option<Token>) -> Result<Self, HubError> {
        let builder = Client::builder()
            .redirect(redirect::Policy::none())
            .no_proxy()
            .timeout(TIMEOUT);
        #[cfg(unix)]
        let builder = match origin.socket_path() {
            Some(path) => builder.unix_socket(path),
            None => builder,
        };
        let http = builder.build().map_err(|_| HubError::Setup)?;
        Ok(Self {
            origin,
            token,
            http,
            declares_agent: false,
        })
    }

    /// Declares an AI agent on every request and socket, so the hub records its work as an agent's (LOG-016).
    #[must_use]
    pub const fn declaring_agent(mut self) -> Self {
        self.declares_agent = true;
        self
    }

    #[must_use]
    pub const fn declares_agent(&self) -> bool {
        self.declares_agent
    }

    #[must_use]
    pub const fn origin(&self) -> &Origin {
        &self.origin
    }

    #[must_use]
    pub const fn token(&self) -> Option<&Token> {
        self.token.as_ref()
    }

    #[must_use]
    pub fn into_token(self) -> Option<Token> {
        self.token
    }

    /// # Errors
    /// [`HubError`] for a refusal (with its K7 code) or a failed request.
    pub async fn run<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<Q::ResponseData, HubError> {
        self.send(&Q::build_query(variables)).await
    }

    /// For a caller that wipes secrets in the variables after the call.
    ///
    /// # Errors
    /// As [`Hub::run`].
    pub async fn send<V: Serialize, R: DeserializeOwned>(
        &self,
        body: &QueryBody<V>,
    ) -> Result<R, HubError> {
        let (status, answer) = self.post(encode(body)).await?;
        answer::read(status, &answer)
    }

    /// # Errors
    /// [`HubError`] when no JSON answer came; a GraphQL error comes back beside the answer.
    pub async fn send_raw(
        &self,
        body: &serde_json::Value,
    ) -> Result<(serde_json::Value, Option<HubError>), HubError> {
        let (status, answer) = self.post(encode(body)).await?;
        answer::read_raw(status, &answer)
    }

    async fn post(&self, body: Zeroizing<Vec<u8>>) -> Result<(u16, Zeroizing<Vec<u8>>), HubError> {
        let mut request = self
            .http
            .post(self.origin.graphql())
            .header(CONTENT_TYPE, "application/json")
            .header(
                ACCEPT,
                "application/graphql-response+json, application/json",
            )
            .body(Bytes::from_owner(body));
        if let Some(token) = &self.token {
            request = request.header(AUTHORIZATION, bearer(token)?);
        }
        if self.declares_agent {
            request = request.header(ACTOR_HEADER, AGENT);
        }
        let mut response = request
            .send()
            .await
            .map_err(|error| self.unreachable(&error))?;
        let status = response.status().as_u16();
        let mut answer = Zeroizing::new(Vec::new());
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| self.unreachable(&error))?
        {
            if answer.len() + chunk.len() > MAX_ANSWER {
                return Err(HubError::Malformed { status });
            }
            answer.extend_from_slice(&chunk);
        }
        Ok((status, answer))
    }

    /// The request's URL is the public origin; no header is in it.
    fn unreachable(&self, error: &reqwest::Error) -> HubError {
        let reason = if error.is_timeout() {
            "it took longer than 30 seconds".to_owned()
        } else {
            deepest_cause(error)
        };
        HubError::Unreachable {
            origin: self.origin.as_text(),
            reason,
        }
    }
}

fn encode(body: &impl Serialize) -> Zeroizing<Vec<u8>> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(8 * 1024));
    serde_json::to_writer(&mut *bytes, body).expect("a GraphQL request body serialises");
    bytes
}

/// Marked sensitive, so the HTTP stack never shows the token.
fn bearer(token: &Token) -> Result<HeaderValue, HubError> {
    let text = Zeroizing::new(format!("Bearer {}", token.expose()));
    let mut value = HeaderValue::from_str(&text).map_err(|_| HubError::Setup)?;
    value.set_sensitive(true);
    Ok(value)
}

fn deepest_cause(error: &reqwest::Error) -> String {
    let mut cause: &dyn std::error::Error = error;
    while let Some(inner) = cause.source() {
        cause = inner;
    }
    cause.to_string()
}

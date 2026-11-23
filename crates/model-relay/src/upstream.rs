//! The providers that the relay forwards to (Section 7.3). The hosts are fixed here. No
//! header and no part of a path can name a different host.

use hyper::HeaderMap;

/// The agent program of a thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Program {
    ClaudeCode,
    Codex,
}

impl Program {
    /// What follows the secret in the base address that the program gets. Codex wants the
    /// `/v1` in its base address; Claude Code adds it.
    #[must_use]
    pub const fn base_path(self) -> &'static str {
        match self {
            Self::ClaudeCode => "",
            Self::Codex => "/v1",
        }
    }
}

/// One host of a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upstream {
    pub(crate) host: String,
    pub(crate) port: u16,
    /// TLS with the web PKI roots. Only a test turns it off, for a listener of its own.
    pub(crate) tls: bool,
    /// What the path of a request starts with at this host.
    pub(crate) prefix: &'static str,
}

impl Upstream {
    const fn provider(host: String, prefix: &'static str) -> Self {
        Self {
            host,
            port: 443,
            tls: true,
            prefix,
        }
    }
}

/// The three hosts of the two providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upstreams {
    pub(crate) anthropic: Upstream,
    pub(crate) openai: Upstream,
    pub(crate) chatgpt: Upstream,
}

/// The header that Codex sends with a `ChatGPT` sign-in, and not with an API key.
const CHATGPT_ACCOUNT: &str = "chatgpt-account-id";

impl Upstreams {
    /// The providers. The only list that the program outside of tests can make.
    #[must_use]
    pub fn providers() -> Self {
        Self {
            anthropic: Upstream::provider("api.anthropic.com".to_owned(), ""),
            openai: Upstream::provider("api.openai.com".to_owned(), "/v1"),
            chatgpt: Upstream::provider("chatgpt.com".to_owned(), "/backend-api/codex"),
        }
    }

    /// Where a request of `program` goes, and its path there. `rest` is the path and the query
    /// after the secret. `None` when the path is not one of that program.
    #[must_use]
    pub fn target(
        &self,
        program: Program,
        headers: &HeaderMap,
        rest: &str,
    ) -> Option<(&Upstream, String)> {
        let (upstream, rest) = match program {
            Program::ClaudeCode => (&self.anthropic, rest),
            Program::Codex => {
                let rest = rest.strip_prefix("/v1")?;
                if headers.contains_key(CHATGPT_ACCOUNT) {
                    (&self.chatgpt, rest)
                } else {
                    (&self.openai, rest)
                }
            }
        };
        (rest.starts_with('/') || rest.is_empty() || rest.starts_with('?'))
            .then(|| (upstream, format!("{}{rest}", upstream.prefix)))
    }
}

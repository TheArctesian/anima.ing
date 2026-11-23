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

/// How the relay knows that the host is the provider. Outside of tests there is one way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Trust {
    /// TLS, with a certificate that the public roots sign.
    WebRoots,
    /// TLS, with this one certificate (DER): a provider that a test plays.
    #[cfg(test)]
    Root(Vec<u8>),
    /// No TLS: a provider that a test plays.
    #[cfg(test)]
    Plain,
}

/// One host of a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upstream {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) trust: Trust,
    /// What the path of a request starts with at this host.
    pub(crate) prefix: &'static str,
}

impl Upstream {
    const fn provider(host: String, prefix: &'static str) -> Self {
        Self {
            host,
            port: 443,
            trust: Trust::WebRoots,
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
    /// after the secret. `None` when the path is not one of that program, or has a `..` part.
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
        let path = rest.split('?').next().unwrap_or(rest);
        // A `..` could leave the part of the host that the prefix names. A host can read
        // `%2e` as a dot and `\\` as `/`, so both are looked at.
        let leaves = |part: &str| part.replace("%2e", ".").replace("%2E", ".") == "..";
        let stays = !path.contains('\\') && !path.split('/').any(leaves);
        let starts = rest.starts_with('/') || rest.is_empty() || rest.starts_with('?');
        (stays && starts).then(|| (upstream, format!("{}{rest}", upstream.prefix)))
    }
}

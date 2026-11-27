//! Dedicated wildcard authority for untrusted development applications.
use super::{Error, origin};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewOrigins {
    authority: String,
}
impl PreviewOrigins {
    pub(super) fn parse(text: &str, hub: &str) -> Result<Self, Error> {
        const KEY: &str = "ANIMA_PREVIEW_ORIGIN";
        let origin = origin::canonical(KEY, text)?;
        let authority = origin
            .strip_prefix("https://")
            .ok_or_else(|| Error::Origin {
                variable: KEY,
                value: origin.clone(),
                reason: "preview origins require HTTPS",
            })?;
        let host = authority.split(':').next().unwrap_or_default();
        let hub_authority = hub.split_once("://").map_or(hub, |(_, value)| value);
        let hub_host = hub_authority.split(':').next().unwrap_or_default();
        if host.parse::<std::net::IpAddr>().is_ok()
            || authority.starts_with('[')
            || host.len() > 216
            || hub_host == host
            || hub_host.ends_with(&format!(".{host}"))
        {
            return Err(Error::Origin {
                variable: KEY,
                value: origin,
                reason: "use a dedicated DNS suffix outside the hub's own host namespace",
            });
        }
        Ok(Self {
            authority: authority.to_owned(),
        })
    }
    #[must_use]
    pub fn origin(&self, endpoint: Uuid) -> String {
        format!("https://{}.{}", endpoint.simple(), self.authority)
    }
    #[must_use]
    pub fn contains(&self, authority: &str) -> bool {
        authority == self.authority || authority.ends_with(&format!(".{}", self.authority))
    }
    /// Exact, canonical Host matching prevents sibling and authority-confusion routing.
    #[must_use]
    pub fn endpoint(&self, authority: &str) -> Option<Uuid> {
        let label = authority.strip_suffix(&format!(".{}", self.authority))?;
        if label.len() != 32
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return None;
        }
        let id = Uuid::parse_str(label).ok()?;
        (!id.is_nil()).then_some(id)
    }
}

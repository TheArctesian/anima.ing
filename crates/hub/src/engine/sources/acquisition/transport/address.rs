//! The URL of a request, refused when its path could leave the declared base (K-E1 Section 7).

use reqwest::Url;

use super::Fault;

/// The URL parser resolves `.`, `..` and `%2e` segments, so the token would go to another path.
fn path_is_plain(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && !path.contains("://")
        && !path
            .chars()
            .any(|c| matches!(c, '@' | '?' | '#' | '\\') || c.is_control() || c.is_whitespace())
        && !path
            .split('/')
            .any(|segment| segment == "." || segment == "..")
        && !path
            .as_bytes()
            .windows(3)
            .any(|w| w[0] == b'%' && w[1] == b'2' && w[2].eq_ignore_ascii_case(&b'e'))
}

/// # Errors
/// [`Fault::Refused`] when the path could leave the base, or the scheme, host, port or path of
/// the result is not the base's.
pub fn address(base: &Url, path: &str, query: &[(String, String)]) -> Result<Url, Fault> {
    if !path_is_plain(path) || !base.username().is_empty() || base.password().is_some() {
        return Err(Fault::Refused);
    }
    let prefix = base.path().trim_end_matches('/');
    let mut url = base.clone();
    url.set_path(&format!("{prefix}{path}"));
    url.set_query(None);
    url.set_fragment(None);
    if !query.is_empty() {
        url.query_pairs_mut().extend_pairs(query);
    }
    let same = url.scheme() == base.scheme()
        && url.host_str() == base.host_str()
        && url.port_or_known_default() == base.port_or_known_default()
        && url
            .path()
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'));
    if same { Ok(url) } else { Err(Fault::Refused) }
}

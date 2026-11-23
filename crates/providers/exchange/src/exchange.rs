//! One request, one reply and what the provider made of it (SRC-006).

use serde_json::Value;

/// The only method a provider can ask for: it cannot express a write (PRJ-017).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
}

/// A path the engine would refuse to send.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    #[error("the path must start with a single slash")]
    NotAbsolutePath,
    #[error("the path must not name a host or hold credentials")]
    NamesAHost,
    #[error("the path must not hold a dot segment or an encoded dot")]
    DotSegment,
}

/// What a provider wants fetched, below the source's `api_base`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub query: Vec<(String, String)>,
}

impl Request {
    /// A `GET` of `path` with `query`; the engine's URL is always `api_base` plus this path.
    ///
    /// # Errors
    /// [`RequestError`] when the path could point away from `api_base`.
    pub fn get(path: &str, query: &[(&str, &str)]) -> Result<Self, RequestError> {
        if !path.starts_with('/') || path.starts_with("//") {
            return Err(RequestError::NotAbsolutePath);
        }
        if path.contains("://") || path.contains('@') {
            return Err(RequestError::NamesAHost);
        }
        if has_dot_segment(path) {
            return Err(RequestError::DotSegment);
        }
        Ok(Self {
            method: Method::Get,
            path: path.to_owned(),
            query: query
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        })
    }
}

/// Whether `path` has a `.` or `..` segment or an encoded dot, either of which a URL parser resolves.
fn has_dot_segment(path: &str) -> bool {
    path.split('/')
        .any(|segment| segment == "." || segment == "..")
        || path
            .as_bytes()
            .windows(3)
            .any(|w| w[0] == b'%' && w[1] == b'2' && w[2].eq_ignore_ascii_case(&b'e'))
}

/// What the engine got back. The provider only reads it.
#[derive(Clone, Copy, Debug)]
pub struct Response<'a> {
    pub status: u16,
    pub body: Body<'a>,
    pub link: Option<&'a str>,
}

/// The body of a reply, as far as the engine read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body<'a> {
    Read(&'a [u8]),
    /// The body was larger than the hub reads, and none of it was kept.
    TooLarge,
}

/// Something wrong the operator should read, in fixed words, never a provider reply (LOG-007).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub code: &'static str,
    pub subject: String,
    pub message: String,
}

/// What a provider made of one response: records, its new slice of state and problems.
#[derive(Clone, Debug, PartialEq)]
pub struct Normalised<R> {
    pub records: Vec<R>,
    pub state: Value,
    pub problems: Vec<Problem>,
}

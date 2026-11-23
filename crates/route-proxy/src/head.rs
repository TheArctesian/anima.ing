//! The head of one HTTP/1.1 request, written again before it goes to the application (Section
//! 12.3, SF-229, SF-332). Pure. One request goes on each connection to the application
//! (`Connection: close`), so each request of the client is read here (SF-326).
//! The `Host` must be the TLS SNI name or the answer is 421; a body has one digits-only `Content-Length`, never with a `Transfer-Encoding`.

use std::net::IpAddr;

pub const HEAD_MAX: usize = 16 * 1024;
const HEADERS_MAX: usize = 100;
/// Headers that the client may not set: the proxy sets them, or the hub or the door uses them.
const DROPPED: [&str; 7] = [
    "forwarded",
    "x-real-ip",
    "connection",
    "keep-alive",
    "proxy-connection",
    "te",
    "trailer",
];
const DROPPED_PREFIXES: [&str; 2] = ["x-forwarded-", "x-anima-"];
/// The prefixes of the names of the cookies of a hub (`__Host-anima`, R-111).
const HUB_COOKIES: [&str; 2] = ["__host-anima", "anima_"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    BadRequest = 400,
    RequestTimeout = 408,
    LengthRequired = 411,
    Misdirected = 421,
    HeadTooLarge = 431,
    NotImplemented = 501,
}

impl Status {
    #[must_use]
    pub const fn code(self) -> u16 {
        self as u16
    }

    const fn reason(self) -> &'static str {
        match self {
            Self::BadRequest => "Bad Request",
            Self::RequestTimeout => "Request Timeout",
            Self::LengthRequired => "Length Required",
            Self::Misdirected => "Misdirected Request",
            Self::HeadTooLarge => "Request Header Fields Too Large",
            Self::NotImplemented => "Not Implemented",
        }
    }

    #[must_use]
    pub fn answer(self) -> Vec<u8> {
        format!(
            "HTTP/1.1 {} {}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
            self.code(),
            self.reason()
        )
        .into_bytes()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rewritten {
    pub head: Vec<u8>,
    /// The bytes of the body that follow the head.
    pub body: u64,
    /// A WebSocket upgrade with its key: after a `101` the bytes go both ways as they are.
    pub upgrade: bool,
    /// The path of the request, for the redirect of port 80 only. Never logged.
    pub target: String,
}

/// The end of the head in `bytes`: the index after `\r\n\r\n`.
#[must_use]
pub fn end_of_head(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|at| at + 4)
}

/// # Errors
/// The [`Status`] that the proxy answers in place of the application.
pub fn rewrite(head: &[u8], sni: &str, client: IpAddr) -> Result<Rewritten, Status> {
    let text = std::str::from_utf8(head).map_err(|_| Status::BadRequest)?;
    let mut lines = text.split("\r\n").filter(|line| !line.is_empty());
    let request_line = lines.next().ok_or(Status::BadRequest)?;
    let target = request_target(request_line)?;
    let mut kept = vec![request_line.to_owned()];
    let mut framing = Framing::default();
    for (count, line) in lines.enumerate() {
        if count >= HEADERS_MAX {
            return Err(Status::HeadTooLarge);
        }
        let (name, value) = header(line)?;
        let lower = name.to_ascii_lowercase();
        if lower == "cookie" {
            if let Some(cookies) = without_hub_cookies(value) {
                kept.push(format!("{name}: {cookies}"));
            }
            continue;
        }
        framing.note(&lower, value)?;
        if !dropped(&lower) {
            kept.push(format!("{name}: {value}"));
        }
    }
    if !framing
        .host
        .as_deref()
        .is_some_and(|host| same_host(host, sni))
    {
        return Err(Status::Misdirected);
    }
    let (body, upgrade) = framing.body()?;
    kept.push(format!("x-forwarded-for: {client}"));
    kept.push("x-forwarded-proto: https".to_owned());
    kept.push(
        if upgrade {
            "connection: upgrade"
        } else {
            "connection: close"
        }
        .to_owned(),
    );
    let mut head = kept.join("\r\n");
    head.push_str("\r\n\r\n");
    Ok(Rewritten {
        head: head.into_bytes(),
        body,
        upgrade,
        target,
    })
}

#[derive(Default)]
struct Framing {
    host: Option<String>,
    length: Option<u64>,
    chunked: bool,
    websocket: bool,
    key: bool,
}

impl Framing {
    fn note(&mut self, lower: &str, value: &str) -> Result<(), Status> {
        match lower {
            "host" | "content-length" if self.seen(lower) => return Err(Status::BadRequest),
            "host" => self.host = Some(value.to_owned()),
            "content-length" => self.length = Some(content_length(value)?),
            "transfer-encoding" => self.chunked = true,
            "upgrade" => self.websocket = value.eq_ignore_ascii_case("websocket"),
            "sec-websocket-key" => self.key = !value.is_empty(),
            _ => {}
        }
        Ok(())
    }

    fn seen(&self, lower: &str) -> bool {
        if lower == "host" {
            self.host.is_some()
        } else {
            self.length.is_some()
        }
    }

    fn body(&self) -> Result<(u64, bool), Status> {
        match (self.chunked, self.length) {
            (true, Some(_)) => Err(Status::BadRequest),
            (true, None) => Err(Status::LengthRequired),
            (false, length) => {
                let body = length.unwrap_or(0);
                let upgrade = self.websocket && self.key;
                if upgrade && body > 0 {
                    return Err(Status::BadRequest);
                }
                Ok((body, upgrade))
            }
        }
    }
}

fn request_target(line: &str) -> Result<String, Status> {
    let mut parts = line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(Status::BadRequest);
    };
    if !method.bytes().all(|b| b.is_ascii_uppercase()) || method.is_empty() {
        return Err(Status::BadRequest);
    }
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return Err(Status::NotImplemented);
    }
    if !target.starts_with('/') {
        return Err(Status::BadRequest);
    }
    Ok(target.to_owned())
}

fn content_length(value: &str) -> Result<u64, Status> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Status::BadRequest);
    }
    value.parse().map_err(|_| Status::BadRequest)
}

/// A name is an RFC 9110 token without `_`: a server that reads `_` as `-` would take
/// `X_Forwarded_For` for a header that the proxy removes.
fn header(line: &str) -> Result<(&str, &str), Status> {
    let (name, value) = line.split_once(':').ok_or(Status::BadRequest)?;
    let token = |b: u8| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^`|~".contains(&b);
    if name.is_empty() || !name.bytes().all(token) {
        return Err(Status::BadRequest);
    }
    let value = value.trim_matches([' ', '\t']);
    if value.bytes().any(|b| b == 0 || b == b'\r' || b == b'\n') {
        return Err(Status::BadRequest);
    }
    Ok((name, value))
}

fn dropped(lower: &str) -> bool {
    DROPPED.contains(&lower)
        || DROPPED_PREFIXES
            .iter()
            .any(|prefix| lower.starts_with(prefix))
}

fn without_hub_cookies(value: &str) -> Option<String> {
    let kept: Vec<&str> = value
        .split(';')
        .map(str::trim)
        .filter(|pair| !pair.is_empty())
        .filter(|pair| {
            let name = pair
                .split('=')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            !HUB_COOKIES.iter().any(|prefix| name.starts_with(prefix))
        })
        .collect();
    (!kept.is_empty()).then(|| kept.join("; "))
}

fn same_host(host: &str, sni: &str) -> bool {
    let name = host.strip_suffix(":443").unwrap_or(host);
    let name = name.strip_suffix('.').unwrap_or(name);
    name.eq_ignore_ascii_case(sni)
}

//! Strip transport and Anima credentials before crossing into an untrusted workspace.
use super::COOKIE;
use axum::http::{HeaderMap, HeaderValue, header};

pub(super) fn request(headers: &mut HeaderMap, port: u16, websocket: bool) {
    hop(headers, websocket);
    for name in [
        "authorization",
        "proxy-authorization",
        "forwarded",
        "x-forwarded-for",
        "x-forwarded-host",
        "x-forwarded-proto",
        "x-real-ip",
    ] {
        headers.remove(name);
    }
    let cookies = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .map(str::trim)
        .filter(|piece| {
            piece.split_once('=').is_some_and(|(name, _)| {
                !name.starts_with("__Host-anima") && !name.starts_with("__Secure-anima")
            })
        })
        .collect::<Vec<_>>()
        .join("; ");
    headers.remove(header::COOKIE);
    if !cookies.is_empty()
        && let Ok(value) = HeaderValue::from_str(&cookies)
    {
        headers.insert(header::COOKIE, value);
    }
    let authority = format!("localhost:{port}");
    if let Ok(value) = HeaderValue::from_str(&authority) {
        headers.insert(header::HOST, value);
    }
    if headers.contains_key(header::ORIGIN)
        && let Ok(value) = HeaderValue::from_str(&format!("http://{authority}"))
    {
        headers.insert(header::ORIGIN, value);
    }
    headers.remove(header::REFERER);
    if !websocket {
        headers.insert(header::CONNECTION, HeaderValue::from_static("close"));
    }
}
pub(super) fn response(headers: &mut HeaderMap, port: u16, origin: &str, websocket: bool) {
    hop(headers, websocket);
    headers.remove("alt-svc");
    headers.remove("clear-site-data");
    headers.insert("origin-agent-cluster", HeaderValue::from_static("?1"));
    headers.insert(
        "cross-origin-opener-policy",
        HeaderValue::from_static("same-origin"),
    );
    response_cookies(headers);
    if let Some(location) = headers
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
    {
        for host in ["localhost", "127.0.0.1"] {
            let base = format!("http://{host}:{port}");
            if let Some(rest) = location.strip_prefix(&base)
                && (rest.is_empty() || rest.starts_with(['/', '?', '#']))
            {
                if let Ok(value) = HeaderValue::from_str(&format!("{origin}{rest}")) {
                    headers.insert(header::LOCATION, value);
                }
                break;
            }
        }
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
}
fn hop(headers: &mut HeaderMap, websocket: bool) {
    let named: Vec<String> = headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(|value| value.trim().to_ascii_lowercase())
        .collect();
    for name in named {
        if !(websocket && name == "upgrade") {
            headers.remove(name);
        }
    }
    for name in [
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailer",
        "transfer-encoding",
    ] {
        headers.remove(name);
    }
    if websocket {
        headers.insert(header::CONNECTION, HeaderValue::from_static("upgrade"));
    } else {
        headers.remove(header::UPGRADE);
    }
}

fn response_cookies(headers: &mut HeaderMap) {
    let cookies: Vec<_> = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| {
            let text = value.to_str().ok()?;
            let mut pieces = text.split(';');
            let first = pieces.next()?;
            let name = first.split_once('=')?.0.trim();
            if name == COOKIE
                || name.starts_with("__Host-anima")
                || name.starts_with("__Secure-anima")
            {
                return None;
            }
            let attrs = pieces.filter(|piece| {
                !piece
                    .trim()
                    .split('=')
                    .next()
                    .is_some_and(|name| name.eq_ignore_ascii_case("domain"))
            });
            HeaderValue::from_str(
                &std::iter::once(first)
                    .chain(attrs)
                    .collect::<Vec<_>>()
                    .join(";"),
            )
            .ok()
        })
        .collect();
    headers.remove(header::SET_COOKIE);
    for cookie in cookies {
        headers.append(header::SET_COOKIE, cookie);
    }
}

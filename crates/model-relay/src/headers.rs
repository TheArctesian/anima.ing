//! The headers that pass. All of them pass as they are, but the ones that belong to one
//! connection and `host`, which the relay sets to the provider.

use hyper::HeaderMap;
use hyper::header::{
    CONNECTION, CONTENT_LENGTH, HOST, HeaderName, PROXY_AUTHENTICATE, PROXY_AUTHORIZATION, TE,
    TRAILER, TRANSFER_ENCODING, UPGRADE,
};

const ONE_CONNECTION: [HeaderName; 7] = [
    CONNECTION,
    PROXY_AUTHENTICATE,
    PROXY_AUTHORIZATION,
    TE,
    TRAILER,
    TRANSFER_ENCODING,
    UPGRADE,
];

/// The headers of a request or of an answer for the other side: as [`passed`], and without
/// `content-length` when a `transfer-encoding` was named. The body is then sent in parts, and
/// a length that was also sent would not agree with it.
#[must_use]
pub fn passed_on(headers: &HeaderMap) -> HeaderMap {
    let mut passed = passed(headers);
    if headers.contains_key(TRANSFER_ENCODING) {
        passed.remove(CONTENT_LENGTH);
    }
    passed
}

/// `headers` for the other side: without the headers of one connection, without the headers
/// that `connection` names, without `keep-alive` and without `host`.
fn passed(headers: &HeaderMap) -> HeaderMap {
    let named: Vec<String> = headers
        .get_all(CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(|name| name.trim().to_ascii_lowercase())
        .collect();
    let mut passed = HeaderMap::with_capacity(headers.len());
    for (name, value) in headers {
        let stays = !ONE_CONNECTION.contains(name)
            && name != HOST
            && name.as_str() != "x-anima-relay"
            && name.as_str() != "keep-alive"
            && !named.iter().any(|named| named == name.as_str());
        if stays {
            passed.append(name.clone(), value.clone());
        }
    }
    passed
}

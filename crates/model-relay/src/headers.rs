//! The headers that pass. All of them pass as they are, but the ones that belong to one
//! connection and `host`, which the relay sets to the provider.

use hyper::HeaderMap;
use hyper::header::{
    CONNECTION, HOST, HeaderName, PROXY_AUTHENTICATE, PROXY_AUTHORIZATION, TE, TRAILER,
    TRANSFER_ENCODING, UPGRADE,
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

/// `headers` for the other side: without the headers of one connection, without the headers
/// that `connection` names, without `keep-alive` and without `host`.
#[must_use]
pub fn passed(headers: &HeaderMap) -> HeaderMap {
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
            && name.as_str() != "keep-alive"
            && !named.iter().any(|named| named == name.as_str());
        if stays {
            passed.append(name.clone(), value.clone());
        }
    }
    passed
}

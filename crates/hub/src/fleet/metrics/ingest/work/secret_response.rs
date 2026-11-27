//! A one-use daemon secret response with a zeroized body.
use axum::{
    http::{StatusCode, header},
    response::{IntoResponse as _, Response},
};
use bytes::Bytes;
use zeroize::Zeroizing;

use crate::fleet::credentials::Secret;
use chrono::{DateTime, Utc};

struct JsonBytes(Zeroizing<Vec<u8>>);

impl AsRef<[u8]> for JsonBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Body<'a> {
    value: &'a str,
    expires_at: chrono::DateTime<chrono::Utc>,
}

pub(super) fn answer(secret: &Secret, expires_at: DateTime<Utc>) -> Response {
    let Ok(value) = std::str::from_utf8(secret.expose()) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let mut body = Zeroizing::new(Vec::with_capacity(value.len() * 6 + 128));
    let input = Body { value, expires_at };
    if serde_json::to_writer(&mut *body, &input).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let headers = [
        (header::CONTENT_TYPE, "application/json"),
        (header::CACHE_CONTROL, "no-store"),
    ];
    (StatusCode::OK, headers, Bytes::from_owner(JsonBytes(body))).into_response()
}

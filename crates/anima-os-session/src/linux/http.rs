use super::{Error, Result};
use serde_json::{Value, json};
use std::time::Duration;

pub fn client(socket: bool) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(25))
        .connect_timeout(Duration::from_secs(10))
        .no_proxy();
    if let Some(path) = std::env::var_os("SSL_CERT_FILE") {
        let bytes = std::fs::read(path)?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(Error::Refused("Certificate store exceeded its limit."));
        }
        for certificate in reqwest::Certificate::from_pem_bundle(&bytes)? {
            builder = builder.add_root_certificate(certificate);
        }
    }
    if socket {
        builder = builder.unix_socket("/var/lib/anima-daemon/link.sock");
    }
    Ok(builder.build()?)
}

pub async fn body(mut response: reqwest::Response) -> Result<Value> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > 1_048_576 {
            return Err(Error::Refused("Account response exceeded its limit."));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(serde_json::from_slice(&bytes)?)
}

pub async fn post(url: &str, value: &Value, token: Option<&str>, form: bool) -> Result<Value> {
    let mut request = client(false)?.post(url);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = if form {
        request.form(value).send().await?
    } else {
        request.json(value).send().await?
    };
    let status = response.status();
    if status.is_redirection() || status.is_server_error() {
        return Err(Error::Refused(
            "The account service is unavailable. Try again later.",
        ));
    }
    let answer = body(response).await?;
    if !status.is_success() {
        return Ok(
            json!({"failure":answer.get("error").and_then(Value::as_str).unwrap_or("refused")}),
        );
    }
    Ok(answer)
}

pub async fn graph(query: &str, variables: Value, token: Option<&str>) -> Result<Value> {
    let mut request = client(true)?
        .post("http://localhost/graphql")
        .json(&json!({"query":query,"variables":variables}));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await?;
    if !response.status().is_success() {
        return Err(Error::Refused(
            "The hub refused this step. Existing ownership was not replaced.",
        ));
    }
    let body = body(response).await?;
    if body.get("errors").is_some() {
        return Err(Error::Refused(
            "The hub refused this step. Existing ownership was not replaced.",
        ));
    }
    body.get("data")
        .cloned()
        .ok_or(Error::Refused("The hub returned an incomplete response."))
}

pub fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 16384)
        .ok_or(Error::Refused(
            "The account service returned an incomplete response.",
        ))
}

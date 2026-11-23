//! Field rules more than one kind uses (CR-14 items 6 and 7). No rule repeats the value it refused.

use reqwest::Url;

use crate::{api_errors::FieldError, fleet::credentials::Secret};

const URL_MAX: usize = 2048;

#[derive(Default)]
pub(super) struct Problems(Vec<FieldError>);

impl Problems {
    pub(super) fn add(&mut self, field: &'static str, message: impl Into<String>) {
        self.0.push(FieldError::new(field, message));
    }

    pub(super) fn take<T>(&mut self, result: Result<T, FieldError>) -> Option<T> {
        result.map_err(|error| self.0.push(error)).ok()
    }

    pub(super) fn note(&mut self, result: Result<(), FieldError>) {
        if let Err(error) = result {
            self.0.push(error);
        }
    }

    pub(super) fn into_errors(self) -> Vec<FieldError> {
        self.0
    }

    pub(super) fn check(self) -> Result<(), Vec<FieldError>> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(self.0)
        }
    }
}

/// A URL parser drops tabs and line breaks silently, so they are refused before it runs.
pub(super) fn has_blank_or_control(text: &str) -> bool {
    text.chars()
        .any(|c| c.is_control() || c.is_whitespace() || crate::text::is_invisible(c))
}

pub(super) fn https_url(field: &'static str, text: &str) -> Result<Url, FieldError> {
    if text.is_empty() {
        return Err(FieldError::new(field, "Enter a URL."));
    }
    if text.len() > URL_MAX {
        return Err(FieldError::new(
            field,
            format!("Use at most {URL_MAX} characters."),
        ));
    }
    if has_blank_or_control(text) {
        return Err(FieldError::new(field, "Remove spaces and line breaks."));
    }
    let url = Url::parse(text).map_err(|_| FieldError::new(field, "Enter a valid https URL."))?;
    if url.scheme() != "https" {
        return Err(FieldError::new(field, "Use an https URL."));
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(FieldError::new(field, "Enter a URL with a host."));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(FieldError::new(
            field,
            "Remove the user name and password from the URL.",
        ));
    }
    Ok(url)
}

/// Normalised to `https://host[:port]`, so two ways of writing one server compare equal.
pub(super) fn server_origin(field: &'static str, text: &str) -> Result<String, FieldError> {
    let url = https_url(field, text)?;
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err(FieldError::new(
            field,
            "Enter only the server's address, such as https://ntfy.sh.",
        ));
    }
    Ok(origin(&url))
}

pub(super) fn origin(url: &Url) -> String {
    url.origin().ascii_serialization()
}

/// Every secret arrives as a GraphQL string, so this fails only for bytes not from the API.
pub(super) fn secret_text<'a>(
    field: &'static str,
    secret: &'a Secret,
) -> Result<&'a str, FieldError> {
    std::str::from_utf8(secret.expose()).map_err(|_| FieldError::new(field, "Enter text."))
}

/// An empty secret is the update's way to clear an optional secret.
pub(super) fn is_empty(secret: &Secret) -> bool {
    secret.expose().is_empty()
}

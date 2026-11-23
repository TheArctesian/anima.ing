//! The version `anima --version` prints (INS-003, OD-17, BLD-043).

pub const VARIABLE: &str = "ANIMA_VERSION";

#[derive(Debug, PartialEq, Eq)]
pub struct Refused;

/// A release version follows the builder's `Version` rule (BLD-002), so a mistyped variable fails the build.
pub fn chosen<'a>(release: Option<&'a str>, crate_version: &'a str) -> Result<&'a str, Refused> {
    match release {
        None | Some("") => Ok(crate_version),
        Some(text) if is_release(text) => Ok(text),
        Some(_) => Err(Refused),
    }
}

fn is_release(text: &str) -> bool {
    let part = |p: &str| {
        !p.is_empty()
            && p.len() <= 9
            && p.bytes().all(|b| b.is_ascii_digit())
            && (p == "0" || !p.starts_with('0'))
    };
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| part(p))
}

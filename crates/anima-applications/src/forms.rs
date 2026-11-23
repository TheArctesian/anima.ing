//! Each value of a request or the settings that goes into a list of arguments has one of these forms.

pub(crate) fn is_lower_hex(text: &str, length: usize) -> bool {
    text.len() == length
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// A lower-case UUID (K10 Section 6.4.6).
pub(crate) fn is_uuid(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    let lengths = [8, 4, 4, 4, 12];
    parts.len() == lengths.len()
        && parts
            .iter()
            .zip(lengths)
            .all(|(part, length)| is_lower_hex(part, length))
}

pub(crate) fn is_digest(text: &str) -> bool {
    text.strip_prefix("sha256:")
        .is_some_and(|hex| is_lower_hex(hex, 64))
}

/// The contract gives no form for an adopted project's name: this is Compose's, with the length of Section 6.4.4.
pub(crate) fn is_project_name(text: &str) -> bool {
    let first = text
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
    first
        && text.len() <= 63
        && text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte))
}

/// A first letter or digit means the name is never read as an option of a command.
pub(crate) fn is_engine_name(text: &str) -> bool {
    let first = text
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric());
    first
        && text.len() <= 255
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
}

/// An address of a different machine has a scheme, so it is never such a path.
pub(crate) fn is_path(text: &str) -> bool {
    let Some(below) = text.strip_prefix('/') else {
        return false;
    };
    text.len() <= 4096
        && !below.is_empty()
        && !text.bytes().any(|byte| byte.is_ascii_control())
        && below
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// One text for each address: IPv6 in RFC 5952 short form, never IPv4-mapped.
fn is_host(text: &str) -> bool {
    if let Some(inner) = text
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        return inner.parse::<std::net::Ipv6Addr>().is_ok_and(|address| {
            address.to_ipv4_mapped().is_none() && address.to_string() == inner
        });
    }
    if text
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return text
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|address| address.to_string() == text);
    }
    let label_sound = |label: &str| {
        (1..=63).contains(&label.len())
            && label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    };
    let last_is_a_name = text
        .rsplit('.')
        .next()
        .is_some_and(|last| !last.bytes().all(|byte| byte.is_ascii_digit()));
    text.len() <= 253 && text.split('.').all(label_sound) && last_is_a_name
}

/// The port is always written (K10 Section 6.4.2), as `https://hub.example:443`.
pub(crate) fn is_origin(text: &str) -> bool {
    let Some((scheme, rest)) = text.split_once("://") else {
        return false;
    };
    let Some((host, port)) = rest.rsplit_once(':') else {
        return false;
    };
    let port_sound = !port.starts_with('0')
        && port.len() <= 5
        && port.bytes().all(|byte| byte.is_ascii_digit())
        && port.parse::<u16>().is_ok();
    matches!(scheme, "http" | "https") && is_host(host) && port_sound
}

/// `anima-` and the 32 hexadecimal digits of the UUID (K10 Section 6.4.6).
pub(crate) fn project_of(application: &str) -> String {
    format!("anima-{}", application.replace('-', ""))
}

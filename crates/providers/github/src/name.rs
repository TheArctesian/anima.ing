const HEX: &[u8; 16] = b"0123456789ABCDEF";

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("a repository name is owner/name, with letters, digits, dots, dashes and underscores")]
pub(crate) struct UnusableName;

pub(crate) fn check(full_name: &str) -> Result<(), UnusableName> {
    let plain = |part: &str| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    match full_name.split_once('/') {
        Some((owner, name)) if plain(owner) && plain(name) => Ok(()),
        _ => Err(UnusableName),
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("a branch name must not have a segment that is a dot or two dots")]
pub(crate) struct UnusableBranch;

/// A URL parser resolves a `.` or `..` segment and so moves the request off the repository.
pub(crate) fn check_branch(branch: &str) -> Result<(), UnusableBranch> {
    if branch
        .split('/')
        .any(|segment| segment == "." || segment == "..")
    {
        Err(UnusableBranch)
    } else {
        Ok(())
    }
}

/// Keeps `/` and percent-encodes each unsafe byte; takes only a branch that passed [`check_branch`].
pub(crate) fn encode_branch(branch: &str) -> String {
    let mut out = String::with_capacity(branch.len());
    for byte in branch.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(byte >> 4)]));
            out.push(char::from(HEX[usize::from(byte & 0x0F)]));
        }
    }
    out
}

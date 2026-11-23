use super::SnapshotError;

/// # Errors
/// [`SnapshotError::BadRemote`] for an empty name, a leading `-`, a space, a control character
/// or a glob character, which `--remotes=<name>` would match against other remotes.
pub fn checked(name: &str) -> Result<&str, SnapshotError> {
    let plain = !name.is_empty()
        && !name.starts_with('-')
        && !name
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '*' | '?' | '[' | '\\'));
    if plain {
        Ok(name)
    } else {
        Err(SnapshotError::BadRemote {
            name: name.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_017_a_remote_that_reads_as_an_option_or_a_pattern_is_refused() {
        assert_eq!(checked("origin").unwrap(), "origin");
        assert_eq!(checked("my-forge.2").unwrap(), "my-forge.2");
        for name in [
            "",
            "-u",
            "--receive-pack=x",
            "a b",
            "a\nb",
            "o*",
            "orig?n",
            "[o]rigin",
            "a\\b",
        ] {
            assert!(checked(name).is_err(), "{name:?}");
        }
    }
}

//! Git refs that hold a snapshot of a working tree (JOB-017); pure: no git, no clock.

use serde::Serialize;

use super::SnapshotError;

/// Nothing outside this prefix is pushed or deleted.
pub const PREFIX: &str = "refs/anima/wip/";

const ID_LENGTH: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ScratchRef(String);

impl ScratchRef {
    #[must_use]
    pub fn for_commit(commit: &str) -> Option<Self> {
        let id = commit.get(..ID_LENGTH).filter(|_| is_hash(commit))?;
        Some(Self(format!("{PREFIX}{id}")))
    }

    /// # Errors
    /// [`SnapshotError::NotScratchRef`] for any other ref or an unsafe name.
    pub fn parse(text: &str) -> Result<Self, SnapshotError> {
        let name = if text.starts_with("refs/") {
            text.strip_prefix(PREFIX)
        } else {
            Some(text)
        };
        name.filter(|name| is_safe_name(name))
            .map(|name| Self(format!("{PREFIX}{name}")))
            .ok_or_else(|| SnapshotError::NotScratchRef {
                text: text.to_owned(),
            })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[must_use]
pub fn fetch_line(remote: &str, scratch: &ScratchRef) -> String {
    format!("git fetch {remote} {}", scratch.as_str())
}

#[must_use]
pub fn is_hash(text: &str) -> bool {
    text.len() >= ID_LENGTH && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// One ref part that git accepts and no shell or option parser can misread.
fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && !name.starts_with(['.', '-'])
        && !name.contains("..")
        && !std::path::Path::new(name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("lock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn job_017_the_ref_is_the_first_16_hex_characters_of_the_commit() {
        let scratch = ScratchRef::for_commit(COMMIT).unwrap();
        assert_eq!(scratch.as_str(), "refs/anima/wip/0123456789abcdef");
        assert_eq!(ScratchRef::for_commit(COMMIT).unwrap(), scratch);
        assert_eq!(ScratchRef::for_commit("0123"), None);
        assert_eq!(ScratchRef::for_commit("refs/heads/main/0123456789"), None);
    }

    #[test]
    fn job_018_a_ref_or_an_id_names_a_scratch_ref() {
        let full = ScratchRef::parse("refs/anima/wip/0123456789abcdef").unwrap();
        let short = ScratchRef::parse("0123456789abcdef").unwrap();
        assert_eq!(full, short);
        assert_eq!(full.as_str(), "refs/anima/wip/0123456789abcdef");
    }

    #[test]
    fn job_018_a_ref_outside_the_scratch_space_is_refused() {
        for text in [
            "refs/heads/main",
            "refs/tags/v1",
            "refs/anima/wipe/x",
            "refs/anima/wip/",
            "refs/anima/wip/a/b",
            "refs/anima/wip/../heads/main",
            "-delete",
            "--force",
            ".hidden",
            "a..b",
            "x.lock",
            "a b",
            "a:b",
            "+x",
            "",
        ] {
            assert!(
                matches!(
                    ScratchRef::parse(text),
                    Err(SnapshotError::NotScratchRef { .. })
                ),
                "{text}"
            );
        }
    }

    #[test]
    fn job_017_the_fetch_line_names_the_remote_and_the_ref() {
        let scratch = ScratchRef::for_commit(COMMIT).unwrap();
        assert_eq!(
            fetch_line("origin", &scratch),
            "git fetch origin refs/anima/wip/0123456789abcdef"
        );
    }
}

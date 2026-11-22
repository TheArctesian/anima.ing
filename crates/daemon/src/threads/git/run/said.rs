//! The words of `git` as a screen may show them: no credential of a URL, and a limit of size.
//!
//! Pure.

/// The words of `git` with each credential of a URL removed (`https://name:secret@host`), cut
/// to a size a screen can show.
#[must_use]
pub fn without_credentials(said: &str) -> String {
    const MOST: usize = 2000;
    let mut clean = String::with_capacity(said.len().min(MOST));
    let mut rest = said;
    while let Some(at) = rest.find("://") {
        let (before, after) = rest.split_at(at + 3);
        clean.push_str(before);
        let end = after
            .find(|c: char| c.is_whitespace() || c == '/' || c == '\'' || c == '"')
            .unwrap_or(after.len());
        match after[..end].rfind('@') {
            Some(user_end) => {
                clean.push_str("***@");
                rest = &after[user_end + 1..];
            }
            None => rest = after,
        }
    }
    clean.push_str(rest);
    let cut = clean
        .char_indices()
        .nth(MOST)
        .map_or(clean.len(), |(at, _)| at);
    clean.truncate(cut);
    clean.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_credential_in_a_url_is_never_in_the_words_of_an_error() {
        let said = "fatal: unable to access 'https://ada:ghp_SECRET123@github.com/a/b.git/': 403\n\
                    remote: ssh://git@host/x and https://plain.example/y";
        let clean = without_credentials(said);
        assert!(!clean.contains("ghp_SECRET123"), "{clean}");
        assert!(!clean.contains("ada:"), "{clean}");
        assert!(clean.contains("https://***@github.com/a/b.git/"), "{clean}");
        assert!(clean.contains("ssh://***@host/x"), "{clean}");
        assert!(clean.contains("https://plain.example/y"), "{clean}");
    }
}

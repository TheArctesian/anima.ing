use super::challenge;

// Public pinned client metadata with synthetic state and PKCE challenge; no account material.
const URL: &str = "https://claude.com/cai/oauth/authorize?client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e&response_type=code&redirect_uri=https%3A%2F%2Fplatform.claude.com%2Foauth%2Fcode%2Fcallback&code_challenge_method=S256&state=synthetic-state&code_challenge=synthetic-challenge";

#[test]
fn terminal_hyperlink_requires_a_complete_url_before_announcing() {
    let prefix = format!("If the browser didn't open, visit: \x1b]8;;{URL}");
    for end in 0..=prefix.len() {
        assert!(challenge(&prefix.as_bytes()[..end]).is_none());
    }
    let complete = format!("{prefix}\x1b\\Open browser\x1b]8;;\x1b\\\n");
    assert_eq!(
        challenge(complete.as_bytes())
            .as_deref()
            .map(String::as_str),
        Some(URL)
    );
    assert_eq!(
        challenge(format!("{URL}\n").as_bytes())
            .as_deref()
            .map(String::as_str),
        Some(URL)
    );
}

#[test]
fn changed_oauth_destination_or_ambiguous_fields_are_not_forwarded() {
    for invalid in [
        URL.replace("claude.com/cai", "claude.com.evil.invalid/cai"),
        URL.replace("platform.claude.com", "evil.invalid"),
        URL.replace("response_type=code", "response_type=token"),
        URL.replace("S256", "plain"),
        URL.replace("synthetic-state", ""),
        URL.replace("9d1c250a-e61b-44d9-88ed-5944d1962f5e", "other-client"),
        format!("{URL}&state=another-state"),
        format!("{URL}#fragment"),
    ] {
        assert!(challenge(format!("{invalid}\n").as_bytes()).is_none());
    }
}

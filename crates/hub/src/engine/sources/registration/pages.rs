//! The pages the hub serves to the operator's browser during Connect GitHub (K-E1 Section 7a, K7).
//! No value from GitHub or a query string is written into a page; only the manifest form's action and JSON vary, both escaped.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest as _, Sha256};

mod installed;

pub const SCRIPT: &str = "document.forms[0].submit();";

pub const PLAIN_POLICY: &str = "default-src 'none'; style-src 'unsafe-inline'; \
    form-action 'self'; base-uri 'none'; frame-ancestors 'none'";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    LinkEnded,
    SignIn,
    NotFromAnima,
    NothingWaiting,
    NotThisBrowser,
    StepPassed,
    TryAgain,
    Connected,
    Finishing,
    /// A hit on `/installed` from a browser that is not bound: it only wakes the poll.
    MaybeInstalled,
    Failed,
}

impl Notice {
    const fn title(self) -> &'static str {
        match self {
            Self::LinkEnded => "This link has ended",
            Self::SignIn => "Sign in to continue",
            Self::NotFromAnima => "Open this from Anima.ing",
            Self::NothingWaiting => "Nothing is waiting",
            Self::NotThisBrowser => "This page is for another browser",
            Self::StepPassed => "This step is done",
            Self::TryAgain => "Could not finish this step",
            Self::Connected => "GitHub is connected.",
            Self::Finishing | Self::MaybeInstalled => "Finishing the connection",
            Self::Failed => "Could not connect GitHub",
        }
    }

    const fn words(self) -> &'static str {
        match self {
            Self::LinkEnded => {
                "This link was already used or has ended. Go back to Anima.ing and \
                press Connect GitHub again."
            }
            Self::SignIn => "Sign in to Anima.ing in this browser, then open the link again.",
            Self::NotFromAnima => {
                "This request did not come from Anima.ing's page. Open the link \
                from Anima.ing again."
            }
            Self::NothingWaiting => "No GitHub connection is waiting. Start one in Anima.ing.",
            Self::NotThisBrowser => {
                "This page belongs to a GitHub connection in another \
                browser, or the link is old. Go back to Anima.ing."
            }
            Self::StepPassed => "This step is done. Go back to Anima.ing to see what comes next.",
            Self::TryAgain => "Anima.ing could not finish this step. Try again in a moment.",
            Self::Connected => "You can close this tab and go back to Anima.ing.",
            Self::Finishing => {
                "The App is made. Anima.ing finishes the connection when GitHub \
                shows it is installed. You can close this tab."
            }
            Self::MaybeInstalled => {
                "If you installed the App, Anima.ing finishes the connection. \
                You can close this tab."
            }
            Self::Failed => {
                "Anima.ing could not finish connecting GitHub. Go back to Anima.ing to see \
                what to do next."
            }
        }
    }
}

#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

#[must_use]
pub fn notice(notice: Notice) -> String {
    installed::document(notice.title(), notice.words(), "")
}

/// The browser's result when it returns from GitHub (PRJ-039, UR-7).
#[must_use]
pub fn installed(notice: Notice) -> String {
    self::notice(notice)
}

/// The same page whether or not an attempt exists, with no value in it.
#[must_use]
pub fn confirm() -> String {
    installed::document(
        "Connect GitHub to Anima.ing",
        "Anima.ing makes a GitHub App in your account and keeps its key in the hub. Continue in \
         a browser that is signed in to Anima.ing.",
        "<form method=\"post\" action=\"/connect/github/open\">\
         <button type=\"submit\">Continue to GitHub</button></form>",
    )
}

#[must_use]
pub fn to_github(action: &str, manifest: &str) -> String {
    installed::document(
        "Opening GitHub",
        "Sending you to GitHub…",
        &format!(
            "<form method=\"post\" action=\"{}\">\
             <input type=\"hidden\" name=\"manifest\" value=\"{}\">\
             <button type=\"submit\">Continue to GitHub</button></form><script>{SCRIPT}</script>",
            escape(action),
            escape(manifest)
        ),
    )
}

/// Only the hashed [`SCRIPT`] runs, and the form goes only to `web_base` (GitHub).
#[must_use]
pub fn to_github_policy(web_base: &str) -> String {
    let hash = STANDARD.encode(Sha256::digest(SCRIPT.as_bytes()));
    format!(
        "default-src 'none'; script-src 'sha256-{hash}'; style-src 'unsafe-inline'; \
         form-action {web_base}; base-uri 'none'; frame-ancestors 'none'"
    )
}

#[cfg(test)]
mod tests {
    use super::{Notice, confirm, escape, notice, to_github};

    #[test]
    fn prj_039_the_form_escapes_its_action_and_the_manifest() {
        let page = to_github(
            "https://github.com/x?a=1&b=\"2\"",
            r#"{"name":"<script>'x'</script>"}"#,
        );
        assert!(page.contains("action=\"https://github.com/x?a=1&amp;b=&quot;2&quot;\""));
        assert!(
            page.contains("&quot;name&quot;:&quot;&lt;script&gt;&#39;x&#39;&lt;/script&gt;&quot;")
        );
        assert_eq!(
            page.matches("<script>").count(),
            1,
            "only the hub's own script"
        );
        assert_eq!(escape("plain-text_1"), "plain-text_1");
    }

    #[test]
    fn prj_041_the_fixed_pages_hold_no_value() {
        assert!(confirm().contains("action=\"/connect/github/open\""));
        assert!(!confirm().contains("<script"));
        assert!(notice(Notice::LinkEnded).contains("already used or has ended"));
    }
}

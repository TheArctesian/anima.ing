//! The one-time link passes through here only: printed once, opened once (MCP-004, LOG-007).

use std::io::{self, Write};
use std::process::{Command, Stdio};

use crate::output::text::clean_lines;

/// It has no `Debug` and no `Serialize`, so no debug output, `--json` or failure text can hold it.
pub struct OneTimeLink(String);

impl OneTimeLink {
    #[must_use]
    pub const fn new(url: String) -> Self {
        Self(url)
    }

    const fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Debug for OneTimeLink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OneTimeLink(..)")
    }
}

/// The link is the argument of the opener's process and of nothing else.
pub type Opener<'a> = &'a mut dyn FnMut(&str) -> io::Result<()>;

pub struct Report<'a> {
    /// Standard error.
    pub notes: &'a mut dyn Write,
    /// Whether standard error is a terminal: only then is the one-time link asked for.
    pub terminal: bool,
    /// `--json`: progress lines are left out; the link is not.
    pub json: bool,
    /// The opener; `None` for `--no-browser`.
    pub browser: Option<Opener<'a>>,
}

impl Report<'_> {
    /// Cleaned, since the text may come from GitHub.
    pub fn say(&mut self, text: &str) {
        // Standard error has no other place to report its own failure.
        let _ = writeln!(self.notes, "{}", clean_lines(text));
        let _ = self.notes.flush();
    }

    pub fn progress(&mut self, text: &str) {
        if !self.json {
            self.say(text);
        }
    }

    /// A browser that cannot open is not an error: the link is already on screen.
    pub fn link(&mut self, intro: &str, link: &OneTimeLink) {
        let _ = writeln!(self.notes, "{intro}\n  {}", link.expose());
        let _ = self.notes.flush();
        if let Some(open) = self.browser.as_mut() {
            let _ = open(link.expose());
        }
    }

    pub fn page(&mut self, intro: &str, page: &str) {
        self.say(&format!("{intro}\n  {page}"));
    }
}

/// # Errors
/// The error of starting it; its own exit status is not waited for.
pub fn system_browser(url: &str) -> io::Result<()> {
    let program = if cfg!(target_os = "macos") {
        "/usr/bin/open"
    } else {
        "/usr/bin/xdg-open"
    };
    Command::new(program)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
}

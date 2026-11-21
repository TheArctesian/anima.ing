//! How the person lists a thread (TH46; inventory 1.4, 1.6, 1.7): pinned, archived, a title of
//! their own, or removed from the list. Anima keeps this in its own record only: it never changes
//! a session file of Claude Code or Codex.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The most characters of a title that the person gives.
pub const MAX_TITLE: usize = 200;

/// How one thread is listed. A thread that has no listing is listed as its files say.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listing {
    pub thread: Uuid,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub archived: bool,
    /// The title the person gave, in place of the title of the program.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Removed from the list of Anima: the session files stay on disk.
    #[serde(default)]
    pub removed: bool,
}

/// A change of the listing of a thread by the person.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum ListingChange {
    Pin {
        on: bool,
    },
    Archive {
        on: bool,
    },
    /// A new title; `None` gives back the title of the program.
    Rename {
        title: Option<String>,
    },
    /// Removes the thread from the list of Anima. The runner refuses it for a thread that runs.
    Remove,
}

impl Listing {
    /// Whether the listing says nothing that the files do not: it need not be kept.
    #[must_use]
    pub const fn is_plain(&self) -> bool {
        !self.pinned && !self.archived && self.title.is_none() && !self.removed
    }

    /// The listing after `change`. A removed thread keeps no pin, archive or title.
    ///
    /// # Errors
    /// A title that is empty after trimming, longer than [`MAX_TITLE`], or with a control
    /// character or a mark that hides text (THR-125).
    pub fn changed(mut self, change: &ListingChange) -> Result<Self, BadTitle> {
        match change {
            ListingChange::Pin { on } => self.pinned = *on,
            ListingChange::Archive { on } => self.archived = *on,
            ListingChange::Rename { title } => {
                self.title = title.as_deref().map(checked_title).transpose()?;
            }
            ListingChange::Remove => {
                return Ok(Self {
                    thread: self.thread,
                    removed: true,
                    ..Self::default()
                });
            }
        }
        Ok(self)
    }
}

/// A title that the runner does not keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("a title is 1 to {MAX_TITLE} characters on one line, with no hidden marks")]
pub struct BadTitle;

pub(crate) fn checked_title(title: &str) -> Result<String, BadTitle> {
    let title = title.trim();
    let fits = !title.is_empty() && title.chars().count() <= MAX_TITLE;
    let shown = title
        .chars()
        .all(|each| !each.is_control() && !crate::marks::is_hidden_mark(each));
    if fits && shown {
        Ok(title.to_owned())
    } else {
        Err(BadTitle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain() -> Listing {
        Listing {
            thread: Uuid::nil(),
            ..Listing::default()
        }
    }

    #[test]
    fn th46_a_title_is_trimmed_and_a_hidden_or_long_title_is_refused() {
        let named = plain()
            .changed(&ListingChange::Rename {
                title: Some("  Fix the login  ".to_owned()),
            })
            .expect("a title");
        assert_eq!(named.title.as_deref(), Some("Fix the login"));
        for bad in [
            "",
            "   ",
            "a\nb",
            "pay\u{202E}evil",
            &"x".repeat(MAX_TITLE + 1),
        ] {
            let change = ListingChange::Rename {
                title: Some(bad.to_owned()),
            };
            assert_eq!(plain().changed(&change), Err(BadTitle), "{bad:?}");
        }
        let back = named
            .changed(&ListingChange::Rename { title: None })
            .expect("no title");
        assert!(back.is_plain());
    }

    #[test]
    fn th46_a_removed_thread_keeps_no_pin_archive_or_title() {
        let marked = plain()
            .changed(&ListingChange::Pin { on: true })
            .and_then(|listing| listing.changed(&ListingChange::Archive { on: true }))
            .expect("marked");
        let removed = marked.changed(&ListingChange::Remove).expect("removed");
        assert!(removed.removed && !removed.pinned && !removed.archived);
        assert!(!removed.is_plain());
    }
}

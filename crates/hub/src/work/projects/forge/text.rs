//! Text from GitHub, cut to the lengths K1 states. Other people write it: it is passed on as a
//! plain string, never parsed, rendered or logged.

pub const TITLE: usize = 300;
/// The most characters of a body or a comment (Markdown source).
pub const BODY: usize = 20_000;
pub const LABELS: usize = 20;

/// Cut on a character boundary; `true` when it was cut.
#[must_use]
pub fn cut(text: String, most: usize) -> (String, bool) {
    match text.char_indices().nth(most) {
        Some((end, _)) => (text[..end].to_owned(), true),
        None => (text, false),
    }
}

/// [`cut`] without the mark, for fields K1 caps without one.
#[must_use]
pub fn capped(text: String, most: usize) -> String {
    cut(text, most).0
}

#[must_use]
pub fn body(text: Option<String>) -> (Option<String>, bool) {
    match text.map(|text| cut(text, BODY)) {
        Some((body, was_cut)) => (Some(body), was_cut),
        None => (None, false),
    }
}

#[must_use]
pub fn first(mut names: Vec<String>, most: usize) -> Vec<String> {
    names.truncate(most);
    names
}

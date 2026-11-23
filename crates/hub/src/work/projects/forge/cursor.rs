//! K7 cursors over the pages of GitHub (API-006).
//! A hub page is cut from one GitHub page, so it costs at most one request and can hold fewer
//! than `first` rows with a next page. A row can repeat or be skipped when the list changes.

use std::ops::Range;

use crate::api_errors::FieldError;

pub const GITHUB_PAGE: usize = 100;
const LAST_PAGE: u32 = 10_000;
const PREFIX: &str = "g1";

/// A place in a list: the GitHub page (from 1) and the row on it (from 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub page: u32,
    pub offset: usize,
}

impl Place {
    pub const START: Self = Self { page: 1, offset: 0 };
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cut {
    pub rows: Range<usize>,
    pub cursors: Vec<String>,
    pub has_next_page: bool,
    /// The cursor after the last row; set on an empty page that has a next page too.
    pub end_cursor: Option<String>,
}

/// # Errors
/// A `first` error outside that range.
pub fn page_size(first: i32, most: usize) -> Result<usize, FieldError> {
    match usize::try_from(first) {
        Ok(size) if (1..=most).contains(&size) => Ok(size),
        _ => Err(FieldError::new(
            "first",
            format!("Ask for between 1 and {most} rows per page."),
        )),
    }
}

/// # Errors
/// An `after` error when the text is not a cursor of this module, or was made for another
/// page size.
pub fn start(after: Option<&str>, size: usize) -> Result<Place, FieldError> {
    let Some(after) = after else {
        return Ok(Place::START);
    };
    let bad = || {
        FieldError::new(
            "after",
            "That page cursor is not valid for this page size. Start again from the first page.",
        )
    };
    let mut parts = after.split('.');
    if parts.next() != Some(PREFIX) {
        return Err(bad());
    }
    let mut number = || {
        parts
            .next()
            .filter(|part| {
                (1..=6).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_digit())
            })
            .and_then(|part| part.parse::<u32>().ok())
    };
    let (page, offset, made_for) = (number(), number(), number());
    let (Some(page), Some(offset), Some(made_for), None) = (page, offset, made_for, parts.next())
    else {
        return Err(bad());
    };
    let offset = usize::try_from(offset).map_err(|_| bad())?;
    let fits = (1..=LAST_PAGE).contains(&page) && offset <= GITHUB_PAGE;
    if !fits || usize::try_from(made_for).ok() != Some(size) {
        return Err(bad());
    }
    Ok(Place { page, offset })
}

#[must_use]
pub fn encode(place: Place, size: usize) -> String {
    format!("{PREFIX}.{}.{}.{size}", place.page, place.offset)
}

/// `full`: the GitHub page was full, so the list may go on past it.
#[must_use]
pub fn cut(place: Place, size: usize, shown: usize, full: bool) -> Cut {
    let begin = place.offset.min(shown);
    let end = begin.saturating_add(size).min(shown);
    let goes_on = full && place.page < LAST_PAGE;
    let after = |row: usize| {
        if row >= shown && goes_on {
            Place {
                page: place.page + 1,
                offset: 0,
            }
        } else {
            Place {
                page: place.page,
                offset: row,
            }
        }
    };
    let has_next_page = end < shown || goes_on;
    let cursors = (begin + 1..=end)
        .map(|row| encode(after(row), size))
        .collect();
    let end_cursor = (end > begin || has_next_page).then(|| encode(after(end), size));
    Cut {
        rows: begin..end,
        cursors,
        has_next_page,
        end_cursor,
    }
}

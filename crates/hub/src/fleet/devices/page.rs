//! Keyset paging by unique device name (API-006).

use std::fmt::Write;

use super::rules::FieldError;

const CURSOR_PREFIX: &str = "n1.";
const MAX_PAGE_SIZE: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Page<T> {
    pub(super) rows: Vec<T>,
    pub(super) has_next_page: bool,
}

pub(super) fn page_size(first: i32) -> Result<usize, FieldError> {
    match usize::try_from(first) {
        Ok(size) if (1..=MAX_PAGE_SIZE).contains(&size) => Ok(size),
        _ => Err(FieldError::new(
            "first",
            format!("Ask for between 1 and {MAX_PAGE_SIZE} items per page."),
        )),
    }
}

#[must_use]
pub(super) fn encode_cursor(name: &str) -> String {
    let mut cursor = String::from(CURSOR_PREFIX);
    for byte in name.bytes() {
        write!(cursor, "{byte:02x}").expect("writing to a String cannot fail");
    }
    cursor
}

pub(super) fn decode_cursor(cursor: &str) -> Result<String, FieldError> {
    let bad = || {
        FieldError::new(
            "after",
            "That page cursor is not valid. Start again from the first page.",
        )
    };
    let hex = cursor.strip_prefix(CURSOR_PREFIX).ok_or_else(bad)?;
    if !hex.is_ascii() || hex.len() % 2 != 0 {
        return Err(bad());
    }
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|_| bad())?;
    String::from_utf8(bytes).map_err(|_| bad())
}

/// `fetched` was read with limit `size + 1`.
#[must_use]
pub(super) fn split_page<T>(mut fetched: Vec<T>, size: usize) -> Page<T> {
    let has_next_page = fetched.len() > size;
    fetched.truncate(size);
    Page {
        rows: fetched,
        has_next_page,
    }
}

/// `sorted` is in the byte order of the name, as the store's `COLLATE "C"`.
#[must_use]
pub(super) fn page_after<'a, T>(
    sorted: &'a [T],
    name_of: impl Fn(&T) -> &str,
    after: Option<&str>,
    size: usize,
) -> Page<&'a T> {
    let start = after.map_or(0, |name| sorted.partition_point(|row| name_of(row) <= name));
    split_page(sorted[start..].iter().take(size + 1).collect(), size)
}

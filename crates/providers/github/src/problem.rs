//! The words of a problem are fixed, never a piece of GitHub's reply (LOG-007).

use source_exchange::{Normalised, Problem};

use crate::record::Record;
use crate::state::Slice;

pub(crate) fn problem(code: &'static str, subject: &str, message: &str) -> Problem {
    Problem {
        code,
        subject: subject.to_owned(),
        message: message.to_owned(),
    }
}

/// The slice stays as it was, so the next run asks for the same page again.
pub(crate) fn refuse(
    slice: &Slice,
    code: &'static str,
    subject: &str,
    message: &str,
) -> Normalised<Record> {
    Normalised {
        records: Vec::new(),
        state: slice.to_value(),
        problems: vec![problem(code, subject, message)],
    }
}

pub(crate) fn unexpected_status(slice: &Slice, status: u16, subject: &str) -> Normalised<Record> {
    refuse(
        slice,
        "unexpected_status",
        subject,
        &format!("GitHub answered with status {status}"),
    )
}

//! The lines of a log that must stay, with the shape of each line (SHA-001): each line that
//! reports a fault, the trace below it, and the first warning of each shape.

pub mod level;
pub mod same;
pub mod trace;

use std::collections::BTreeSet;

use crate::choice::{Marked, Reason};
use crate::rows::error_words::names_error;
use level::{Level, level};

/// What the choice needs to know of each line.
#[must_use]
pub fn marked_lines(lines: &[&str]) -> Marked {
    let same: Vec<String> = lines.iter().map(|line| same::shape(line)).collect();
    let mut warnings = BTreeSet::new();
    let mut in_trace = 0_usize;
    let mut must = Vec::with_capacity(lines.len());
    for (line, shape) in lines.iter().zip(&same) {
        let reason = if level(line) == Level::Error || names_error(line) {
            in_trace = trace::MOST_LINES;
            Some(Reason::Error)
        } else if in_trace > 0 && trace::continues_trace(line) {
            in_trace -= 1;
            Some(Reason::Trace)
        } else {
            in_trace = 0;
            (level(line) == Level::Warning && warnings.insert(shape.as_str()))
                .then_some(Reason::Warning)
        };
        must.push(reason);
    }
    Marked { must, same }
}

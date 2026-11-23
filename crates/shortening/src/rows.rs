// Parts of this file are from Headroom (github.com/headroomlabs-ai/headroom), Copyright 2025
// Headroom Contributors, licensed under the Apache License, Version 2.0.
// Changed by Anima.ing, 2026: the walk only leaves rows out; it writes no mark in the list and
// changes no value. A list of numbers or of mixed kinds is not touched.
// Source: crates/headroom-core/src/transforms/smart_crusher/crusher.rs (`process_value`,
// `crush_array`) and crates/headroom-core/src/transforms/smart_crusher/classifier.rs

//! The lists of an answer, each cut to the rows that stay (SHA-001, SHA-003).

pub mod error_words;
pub mod numbers;
pub mod rare;

use serde_json::Value;

use crate::choice::{Kept, Marked, Reason, choice};
use crate::limits::Limits;
use crate::lines::marked_lines;

/// The deepest list that is looked at (Headroom: 50).
const DEEPEST: usize = 50;

/// One list that lost rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cut {
    /// Where the list is in the answer, for example `events` or `page.entries`.
    pub list: String,
    /// How many rows the list had.
    pub rows: usize,
    /// The rows that stay, by their place in the whole list, each with its reason.
    pub kept: Kept,
}

/// Leaves out rows of the lists of `answer`, and says which lists lost rows.
pub fn cut_lists(answer: &mut Value, limits: &Limits) -> Vec<Cut> {
    let mut walk = Walk {
        limits,
        cuts: Vec::new(),
    };
    walk.visit(answer, "", 0);
    walk.cuts
}

struct Walk<'a> {
    limits: &'a Limits,
    cuts: Vec<Cut>,
}

impl Walk<'_> {
    fn visit(&mut self, value: &mut Value, path: &str, depth: usize) {
        if depth > DEEPEST {
            return;
        }
        match value {
            Value::Object(fields) => {
                for (name, value) in fields.iter_mut() {
                    let path = if path.is_empty() {
                        name.clone()
                    } else {
                        format!("{path}.{name}")
                    };
                    self.visit(value, &path, depth + 1);
                }
            }
            Value::Array(rows) => {
                if let Some(kept) = marked(rows).and_then(|marked| choice(&marked, self.limits)) {
                    self.cut(rows, path, kept);
                } else {
                    for (row, value) in rows.iter_mut().enumerate() {
                        self.visit(value, &format!("{path}[{row}]"), depth + 1);
                    }
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }

    fn cut(&mut self, rows: &mut Vec<Value>, path: &str, kept: Kept) {
        let total = rows.len();
        let mut place = 0_usize;
        rows.retain(|_| {
            place += 1;
            kept.contains_key(&(place - 1))
        });
        self.cuts.push(Cut {
            list: path.to_owned(),
            rows: total,
            kept,
        });
    }
}

/// What the choice needs to know of each row: for a list of objects and for a list of texts.
fn marked(rows: &[Value]) -> Option<Marked> {
    if rows.iter().all(Value::is_object) {
        let rare = rare::rare_rows(rows);
        let far = numbers::far_rows(rows);
        let must = rows
            .iter()
            .zip(rare.iter().zip(&far))
            .map(|(row, (rare, far))| {
                if error_words::reports_error(row) {
                    Some(Reason::Error)
                } else if *rare {
                    Some(Reason::Rare)
                } else {
                    far.then_some(Reason::Far)
                }
            })
            .collect();
        let same = rows.iter().map(Value::to_string).collect();
        return Some(Marked { must, same });
    }
    let lines: Option<Vec<&str>> = rows.iter().map(Value::as_str).collect();
    lines.map(|lines| marked_lines(&lines))
}

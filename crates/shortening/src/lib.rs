//! The shortener of long answers for an AI agent (SHA-001 to SHA-009): rows of a list in JSON
//! and lines of a log are left out, never a row that reports a fault or that is not like the
//! others, and the answer says that it was shortened.
//!
//! Pure: no file, no network, no clock, no log (SHA-009). The same answer gives the same
//! result (SHA-005). The caller stores the original before it gives the shortened answer.
//!
//! Parts are from Headroom (Apache-2.0); see `NOTICE` and the head of each file.

mod choice;
mod limits;
mod lines;
mod mark;
mod rows;
#[cfg(test)]
mod tests;

use serde_json::Value;

pub use choice::{Kept, Reason};
pub use limits::Limits;
pub use mark::FIELD;
pub use rows::Cut;

/// The bytes that the mark of a shortened answer takes, about, for the test of the saving:
/// the fixed part, and the part of each list.
const MARK_ROOM: usize = 450;
const MARK_ROOM_OF_A_LIST: usize = 160;
/// The most lists of one answer that lose rows. An answer with more is not touched: its mark
/// would be long, and it is not a page of rows.
const MOST_CUTS: usize = 8;

/// An answer with rows left out, before it has its mark.
#[derive(Debug, Clone, PartialEq)]
pub struct Shortened {
    answer: Value,
    cuts: Vec<Cut>,
}

impl Shortened {
    /// The lists that lost rows.
    #[must_use]
    pub fn cuts(&self) -> &[Cut] {
        &self.cuts
    }

    /// The answer with its mark (SHA-004): `handle` names the stored original and `tool` the
    /// tool that gives it.
    #[must_use]
    pub fn marked(self, handle: &str, tool: &str) -> Value {
        let mut answer = self.answer;
        if let Value::Object(fields) = &mut answer {
            fields.insert(FIELD.to_owned(), mark::mark(&self.cuts, handle, tool));
        }
        answer
    }
}

/// `answer` with rows left out; `None` when it stays as it is (SHA-007): it is not an object,
/// it has a field named as the mark, it is small, no list is long, more than 8 lists would
/// lose rows, or the saving is small.
#[must_use]
pub fn shortened(answer: &Value, limits: &Limits) -> Option<Shortened> {
    let fields = answer.as_object()?;
    if fields.contains_key(FIELD) {
        return None;
    }
    let before = answer.to_string().len();
    if before < limits.smallest_answer {
        return None;
    }
    let mut cut = answer.clone();
    let cuts = rows::cut_lists(&mut cut, limits);
    if cuts.is_empty() || cuts.len() > MOST_CUTS {
        return None;
    }
    let after = cut.to_string().len() + MARK_ROOM + MARK_ROOM_OF_A_LIST * cuts.len();
    let saved = before.saturating_sub(after);
    (saved * 100 >= before * limits.least_saving_percent).then_some(Shortened { answer: cut, cuts })
}

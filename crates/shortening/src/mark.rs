//! The mark of a shortened answer (SHA-004).

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::rows::Cut;

/// The field of an answer that holds the mark.
pub const FIELD: &str = "shortened";

/// The mark for `cuts`, with the handle of the original and the tool that gives it.
#[must_use]
pub fn mark(cuts: &[Cut], handle: &str, tool: &str) -> Value {
    let lists: Vec<Value> = cuts.iter().map(list).collect();
    json!({
        "note": format!(
            "Rows were left out of this answer to save context: it is NOT the whole answer. \
             Each row that reports a fault, has a rare field or value, or has a number far \
             from the others was kept. Do not count or sum the rows shown. For the whole \
             answer call {tool} with this handle."
        ),
        "handle": handle,
        "tool": tool,
        "lists": lists,
    })
}

fn list(cut: &Cut) -> Value {
    let mut reasons: BTreeMap<&str, usize> = BTreeMap::new();
    for reason in cut.kept.values() {
        *reasons.entry(reason.name()).or_insert(0) += 1;
    }
    let kept_for: Map<String, Value> = reasons
        .into_iter()
        .map(|(name, count)| (name.to_owned(), json!(count)))
        .collect();
    json!({
        "list": cut.list,
        "rows": cut.rows,
        "kept": cut.kept.len(),
        "leftOut": cut.rows - cut.kept.len(),
        "keptFor": kept_for,
    })
}

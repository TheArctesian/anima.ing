//! The documents select different parts of the K1 `Issue`, so each is rendered from its JSON.

use serde::Serialize;
use serde_json::Value;

use crate::forge::comments;
use crate::forge::render::{flag, many, names, number, text};
use crate::output::{Cell, Table, Tone};

fn state(issue: &Value) -> Cell {
    if flag(issue, "open") {
        return Cell::toned("open", Tone::Good);
    }
    match issue.get("closeReason").and_then(Value::as_str) {
        Some(reason) => Cell::toned(&format!("closed ({reason})"), Tone::Muted),
        None => Cell::toned("closed", Tone::Muted),
    }
}

#[must_use]
pub fn list<T: Serialize>(issues: &T) -> Table {
    let issues = serde_json::to_value(issues).unwrap_or_default();
    let mut table = Table::new(vec![
        "NUMBER", "STATE", "TITLE", "AUTHOR", "LABELS", "COMMENTS",
    ]);
    for edge in many(&issues, "edges") {
        let issue = &edge["node"];
        table.row(vec![
            number(issue, "number").into(),
            state(issue),
            text(issue, "title").into(),
            text(issue, "authorLogin").into(),
            names(issue, "labels").into(),
            number(issue, "comments").into(),
        ]);
    }
    table
}

/// The body is not asked for, so text the operator sent is not printed back.
#[must_use]
pub fn changed<T: Serialize>(issue: &T) -> Table {
    let issue = serde_json::to_value(issue).unwrap_or_default();
    let mut table = Table::record();
    table.pair("Number", number(&issue, "number"));
    table.pair("Title", text(&issue, "title"));
    table.pair("State", state(&issue));
    table.pair("Labels", names(&issue, "labels"));
    table.pair("Link", text(&issue, "url"));
    table
}

#[must_use]
pub fn record<T: Serialize>(issue: &T) -> Table {
    let issue = serde_json::to_value(issue).unwrap_or_default();
    let mut table = Table::record();
    table.pair("Number", number(&issue, "number"));
    table.pair("Title", text(&issue, "title"));
    table.pair("State", state(&issue));
    table.pair("Author", text(&issue, "authorLogin"));
    table.pair("Assignees", names(&issue, "assignees"));
    table.pair("Labels", names(&issue, "labels"));
    table.pair("Comments", number(&issue, "comments"));
    table.pair("Updated", text(&issue, "updatedAt"));
    table.pair("Link", text(&issue, "url"));
    let cut = if flag(&issue, "bodyCut") {
        " (cut)"
    } else {
        ""
    };
    table.pair("Body", format!("{}{cut}", text(&issue, "body")));
    comments::lines(&mut table, &issue);
    table
}

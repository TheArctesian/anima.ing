use agent_threads::control::ThreadRow;
use serde_json::json;

use super::{context_words, folder_words};

fn row(more: &serde_json::Value) -> ThreadRow {
    let mut row = json!({
        "id": "00000000-0000-0000-0000-000000000007", "program": "claude_code", "folder": "/w/side",
        "mode": "ask", "model": null, "state": "idle", "session": null, "title": "t",
        "started": "2026-11-22T10:00:00Z", "ended": null, "open": []
    });
    if let (Some(row), Some(more)) = (row.as_object_mut(), more.as_object()) {
        row.extend(more.clone());
    }
    serde_json::from_value(row).expect("a row")
}

#[test]
fn th112_the_table_says_a_worktree_and_how_full_the_context_is() {
    let worktree = row(&json!({
        "workspace": {"kind": "worktree", "top": "/w/side"},
        "usage": {"context": 50_000, "window": 200_000, "input": 1, "output": 1}
    }));
    assert_eq!(folder_words(&worktree), "/w/side (worktree)");
    assert_eq!(context_words(&worktree), "25%");
    let plain = row(&json!({
        "workspace": {"kind": "checkout", "top": "/w"},
        "usage": {"context": 5, "window": 0, "input": 1, "output": 1}
    }));
    assert_eq!(folder_words(&plain), "/w/side");
    assert_eq!(context_words(&plain), "-", "no window");
    assert_eq!(context_words(&row(&json!({}))), "-", "no use said yet");
}

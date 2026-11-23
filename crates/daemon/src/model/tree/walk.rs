//! The tree of a server as the walk of the parent processes (macOS; R-331).
//! A root is the server or a process an earlier poll saw, so a double-forked child stays once seen.

use std::collections::{BTreeMap, BTreeSet};

use super::Identity;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub pid: i32,
    pub parent: i32,
    pub start: String,
}

#[must_use]
pub fn read_table(text: &str) -> Vec<Row> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let pid = words.next()?.parse().ok()?;
            let parent = words.next()?.parse().ok()?;
            let start = words.collect::<Vec<_>>().join(" ");
            (!start.is_empty()).then_some(Row { pid, parent, start })
        })
        .collect()
}

#[must_use]
pub fn descendants(table: &[Row], roots: &[Identity]) -> Vec<Identity> {
    let by_pid: BTreeMap<i32, &Row> = table.iter().map(|row| (row.pid, row)).collect();
    let mut children: BTreeMap<i32, Vec<&Row>> = BTreeMap::new();
    for row in table {
        children.entry(row.parent).or_default().push(row);
    }
    let mut seen = BTreeSet::new();
    let mut found = Vec::new();
    let mut queue: Vec<&Row> = roots
        .iter()
        .filter_map(|root| {
            by_pid
                .get(&root.pid)
                .filter(|row| row.start == root.start)
                .copied()
        })
        .collect();
    while let Some(row) = queue.pop() {
        if !seen.insert(row.pid) {
            continue;
        }
        found.push(Identity {
            pid: row.pid,
            start: row.start.clone(),
        });
        queue.extend(children.get(&row.pid).into_iter().flatten().copied());
    }
    found
}

fn ps(arguments: &[&str]) -> Option<String> {
    let output = std::process::Command::new("/bin/ps")
        .args(arguments)
        .env_clear()
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[must_use]
pub fn table() -> Option<Vec<Row>> {
    ps(&["-axo", "pid=,ppid=,lstart="]).map(|text| read_table(&text))
}

#[must_use]
pub fn start_of(pid: i32) -> Option<String> {
    let text = ps(&["-o", "lstart=", "-p", &pid.to_string()])?;
    let start = text.trim();
    (!start.is_empty()).then(|| start.split_whitespace().collect::<Vec<_>>().join(" "))
}

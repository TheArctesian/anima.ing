use serde::Deserialize;

const MAX_EVENTS: usize = 512;

#[derive(Deserialize)]
struct Entry {
    sid: String,
    #[serde(flatten)]
    event: Event,
}

#[derive(Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum Event {
    CmdName {
        name: String,
    },
    ChildStart {
        child_id: u32,
        child_class: String,
        use_shell: bool,
    },
    ChildExit {
        child_id: u32,
        pid: u32,
        code: i32,
    },
    Data {
        category: String,
        key: String,
        value: serde_json::Value,
    },
    Exit {
        code: i32,
    },
    #[serde(other)]
    Other,
}

/// Git sends the JSON trace through the bounded diagnostic pipe. A negotiation is evidence
/// only if the clone launched the HTTP helper, linked by the child IDs and process IDs.
pub(super) fn negotiated(trace: &[u8], pid: u32, transport: &str) -> bool {
    verified(trace, pid, transport).is_some()
}

fn verified(trace: &[u8], pid: u32, transport: &str) -> Option<()> {
    let entries = read(trace)?;
    let (clone, name) = command(&entries, "", pid)?;
    if name != "clone" || !ended(&entries, clone) || !one_session(&entries, clone) {
        return None;
    }
    let (child, name) = child(&entries, clone, transport)?;
    let helper = match name {
        "remote-curl" => child,
        "_run_dashed_" => {
            let (helper, "remote-curl") = self::child(&entries, child, "dashed")? else {
                return None;
            };
            helper
        }
        _ => return None,
    };
    if !ended(&entries, helper) {
        return None;
    }
    let version = only(entries.iter().filter_map(|entry| match &entry.event {
        Event::Data {
            category,
            key,
            value,
        } if entry.sid == helper && category == "transfer" && key == "negotiated-version" => {
            Some(value)
        }
        _ => None,
    }))?;
    matches!(version.as_str(), Some("0" | "1" | "2")).then_some(())
}

fn one_session(entries: &[Entry], root: &str) -> bool {
    entries.iter().all(|entry| {
        entry.sid == root
            || entry
                .sid
                .strip_prefix(root)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

fn read(trace: &[u8]) -> Option<Vec<Entry>> {
    if !trace.ends_with(b"\n") {
        return None;
    }
    let mut entries = Vec::new();
    let text = std::str::from_utf8(trace).ok()?;
    for (at, line) in text.lines().enumerate() {
        if at >= MAX_EVENTS {
            return None;
        }
        // Git also writes warnings to this pipe. Evidence must be a complete JSON line.
        if line.trim_start().starts_with('{') {
            entries.push(serde_json::from_str(line).ok()?);
        }
    }
    Some(entries)
}

fn command<'a>(entries: &'a [Entry], parent: &str, pid: u32) -> Option<(&'a str, &'a str)> {
    only(entries.iter().filter_map(|entry| {
        let Event::CmdName { name } = &entry.event else {
            return None;
        };
        let sid = if parent.is_empty() {
            &entry.sid
        } else {
            entry.sid.strip_prefix(parent)?.strip_prefix('/')?
        };
        let (_, hex) = sid.rsplit_once("-P")?;
        (!sid.contains('/') && hex.len() == 8 && u32::from_str_radix(hex, 16).ok()? == pid)
            .then_some((entry.sid.as_str(), name.as_str()))
    }))
}

fn child<'a>(entries: &'a [Entry], parent: &str, class: &str) -> Option<(&'a str, &'a str)> {
    let (id, use_shell) = only(entries.iter().filter_map(|entry| match &entry.event {
        Event::ChildStart {
            child_id,
            child_class,
            use_shell,
        } if entry.sid == parent && child_class == class => Some((*child_id, *use_shell)),
        _ => None,
    }))?;
    if use_shell {
        return None;
    }
    let (pid, code) = only(entries.iter().filter_map(|entry| match entry.event {
        Event::ChildExit {
            child_id,
            pid,
            code,
        } if entry.sid == parent && child_id == id => Some((pid, code)),
        _ => None,
    }))?;
    if code == 0 {
        command(entries, parent, pid)
    } else {
        None
    }
}

fn ended(entries: &[Entry], sid: &str) -> bool {
    only(entries.iter().filter_map(|entry| match entry.event {
        Event::Exit { code } if entry.sid == sid => Some(code),
        _ => None,
    })) == Some(0)
}

fn only<T>(mut found: impl Iterator<Item = T>) -> Option<T> {
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

#[cfg(test)]
mod tests;

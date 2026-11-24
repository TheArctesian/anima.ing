//! Group members on macOS from `dscl`, only when the search policy reads the local directory
//! alone. Nested groups, unknown identifiers and calculated groups count as unread.

use std::collections::HashMap;
use std::path::Path;

use nix::unistd::Group;

use super::super::Member;
use super::{ask, by_name, number};

const DSCL: &str = "/usr/bin/dscl";

/// `everyone`, `authedusers`, `interactusers`, `netusers`, `consoleusers`, `localaccounts`,
/// `netaccounts`: the system calculates their members.
const CALCULATED: [u32; 7] = [12, 50, 51, 52, 53, 61, 62];

const LOCAL_NODES: [&str; 2] = ["/Local/Default", "/BSD/Local"];

pub(super) fn members(group: &Group) -> Option<Vec<Member>> {
    let number = group.gid.as_raw();
    let dscl = Path::new(DSCL);
    if CALCULATED.contains(&number)
        || !local_search(&ask(dscl, &["/Search", "-read", "/", "SearchPath"])?)
    {
        return None;
    }
    let name = plain(&group.name)?;
    let place = format!("/Groups/{name}");
    let keys = [
        "PrimaryGroupID",
        "GroupMembership",
        "GroupMembers",
        "NestedGroups",
    ];
    let mut arguments = vec!["/Search", "-read", place.as_str()];
    arguments.extend(keys);
    let (mut names, identifiers) = record(&ask(dscl, &arguments)?, number)?;
    let users = ask(dscl, &["/Search", "-list", "/Users", "GeneratedUID"])?;
    names.extend(resolve(&identifiers, &users)?);
    let users = ask(dscl, &["/Search", "-list", "/Users", "PrimaryGroupID"])?;
    names.extend(primary(&users, number)?);
    names.extend(group.mem.iter().cloned());
    by_name(names)
}

fn plain(name: &str) -> Option<&str> {
    (!name.is_empty() && !name.starts_with('-') && !name.contains('/')).then_some(name)
}

pub(in super::super) fn local_search(text: &str) -> bool {
    let mut words = text.split_whitespace();
    let key = words.next();
    let nodes: Vec<&str> = words.collect();
    key == Some("SearchPath:")
        && !nodes.is_empty()
        && nodes.iter().all(|node| LOCAL_NODES.contains(node))
}

fn pairs(text: &str) -> Option<Vec<(&str, &str)>> {
    let mut pairs = Vec::new();
    for line in text.lines() {
        match line.split_whitespace().collect::<Vec<_>>().as_slice() {
            [] | [_] => {}
            [name, value] => pairs.push((*name, *value)),
            _ => return None,
        }
    }
    Some(pairs)
}

/// From `dscl -list /Users PrimaryGroupID`: the name of each account whose own group is `group`.
pub(in super::super) fn primary(text: &str, group: u32) -> Option<Vec<String>> {
    let mut names = Vec::new();
    for (name, own) in pairs(text)? {
        if number(own)? == group {
            names.push(name.to_owned());
        }
    }
    Some(names)
}

/// From `dscl -list /Users GeneratedUID`; `None` when an identifier names no account there.
pub(in super::super) fn resolve(identifiers: &[String], text: &str) -> Option<Vec<String>> {
    let accounts: HashMap<&str, &str> = pairs(text)?
        .into_iter()
        .map(|(name, identifier)| (identifier, name))
        .collect();
    identifiers
        .iter()
        .map(|identifier| {
            accounts
                .get(identifier.as_str())
                .map(|name| (*name).to_owned())
        })
        .collect()
}

/// From `dscl -read` of the group.
pub(in super::super) fn record(text: &str, group: u32) -> Option<(Vec<String>, Vec<String>)> {
    let mut values: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut key = None;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        // A value on the next line, after a space, when it is long or holds a space.
        let (name, rest) = match line.strip_prefix(' ') {
            Some(rest) => (key?, rest),
            None => line.split_once(':')?,
        };
        key = Some(name);
        values
            .entry(name)
            .or_default()
            .extend(rest.split_whitespace());
    }
    let numbers = values.get("PrimaryGroupID").map(Vec::as_slice);
    let same = matches!(numbers, Some([own]) if number(own) == Some(group));
    if !same || values.contains_key("NestedGroups") {
        return None;
    }
    let owned = |list: Vec<&str>| -> Vec<String> { list.into_iter().map(str::to_owned).collect() };
    let names = owned(values.remove("GroupMembership")?);
    let identifiers = owned(values.remove("GroupMembers").unwrap_or_default());
    Some((names, identifiers))
}

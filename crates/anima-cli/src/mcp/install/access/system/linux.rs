//! Group members on Linux from `getent`, only when `nsswitch.conf` takes accounts and groups from
//! files and systemd alone, which list each of them; a private group is read in each case.

use std::fs;
use std::io;
use std::path::Path;

use nix::unistd::{Group, Uid, User};

use super::super::Member;
use super::{ask, by_name, number};

/// The last is NixOS.
const GETENT: [&str; 3] = [
    "/usr/bin/getent",
    "/bin/getent",
    "/run/current-system/sw/bin/getent",
];

const NSSWITCH: &str = "/etc/nsswitch.conf";

const LISTED: [&str; 2] = ["files", "systemd"];

pub(super) fn members(group: &Group) -> Option<Vec<Member>> {
    // Only an administrator can give a private group a member that `getent` does not list, so it
    // is read also with a directory service.
    let owner = User::from_uid(Uid::from_raw(group.gid.as_raw()))
        .ok()
        .flatten()
        .filter(|user| user.name == group.name && user.gid == group.gid);
    let private = owner.is_some();
    // With no such file (as with musl), the system reads only its files.
    match fs::read_to_string(NSSWITCH) {
        Ok(text) => (private || local_only(&text)).then_some(())?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return None,
    }
    let getent = GETENT
        .into_iter()
        .map(Path::new)
        .find(|path| path.exists())?;
    let number = group.gid.as_raw();
    let mut members = by_name(entries(&ask(getent, &["group"])?, number)?)?;
    members.extend(passwd(&ask(getent, &["passwd"])?, number)?);
    // The account whose private group it is, also when `getent passwd` does not list it.
    members.extend(owner.map(|user| Member {
        name: user.name,
        uid: Some(user.uid.as_raw()),
    }));
    Some(members)
}

pub(in super::super) fn local_only(text: &str) -> bool {
    let lines = |database: &str| -> Vec<&str> {
        text.lines()
            .map(|line| line.split('#').next().unwrap_or_default().trim())
            .filter_map(|line| {
                // glibc ends the name of the database at a colon or a blank.
                let rest = line.strip_prefix(database)?;
                rest.starts_with(|part: char| part == ':' || part.is_whitespace())
                    .then(|| rest.trim_start().trim_start_matches(':'))
            })
            .collect()
    };
    let listed = |line: &&str| {
        let mut sources = line
            .split_whitespace()
            .filter(|word| !(word.starts_with('[') && word.ends_with(']')));
        sources.clone().next().is_some() && sources.all(|source| LISTED.contains(&source))
    };
    let initgroups = lines("initgroups");
    ["passwd", "group"]
        .iter()
        .all(|database| matches!(lines(database).as_slice(), [line] if listed(line)))
        && initgroups.len() <= 1
        && initgroups.iter().all(listed)
}

/// A number can have more than one entry in `getent group`.
pub(in super::super) fn entries(text: &str, group: u32) -> Option<Vec<String>> {
    let mut names = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let fields: Vec<&str> = line.splitn(4, ':').collect();
        let [_, _, own, listed] = fields.as_slice() else {
            return None;
        };
        if number(own)? == group {
            names.extend(
                listed
                    .split(',')
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned),
            );
        }
    }
    Some(names)
}

pub(in super::super) fn passwd(text: &str, group: u32) -> Option<Vec<Member>> {
    let mut members = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let fields: Vec<&str> = line.splitn(5, ':').collect();
        let [name, _, uid, own, ..] = fields.as_slice() else {
            return None;
        };
        let uid = number(uid)?;
        if number(own)? == group {
            members.push(Member {
                name: (*name).to_owned(),
                uid: Some(uid),
            });
        }
    }
    Some(members)
}

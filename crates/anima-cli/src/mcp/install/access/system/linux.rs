//! The members of a group on Linux: each entry of `getent group` with its number, and each account
//! of `getent passwd` whose own group it is. Only when `/etc/nsswitch.conf` takes accounts and
//! groups from the files of this computer and from systemd, which list each of them: a directory
//! service (SSSD, LDAP, winbind) need not.

use std::fs;
use std::io;
use std::path::Path;

use nix::unistd::Group;

use super::super::Member;
use super::{ask, by_name, number};

/// Where Linux systems keep `getent`; the last is NixOS.
const GETENT: [&str; 3] = [
    "/usr/bin/getent",
    "/bin/getent",
    "/run/current-system/sw/bin/getent",
];

/// The file that names where the system takes its accounts and groups.
const NSSWITCH: &str = "/etc/nsswitch.conf";

/// The sources of accounts and groups that list each of them.
const LISTED: [&str; 2] = ["files", "systemd"];

pub(super) fn members(group: &Group) -> Option<Vec<Member>> {
    // With no such file (as with musl), the system reads only its files.
    match fs::read_to_string(NSSWITCH) {
        Ok(text) => local_only(&text).then_some(())?,
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
    Some(members)
}

/// Whether the lines `passwd` and `group` of `nsswitch.conf` name only sources of [`LISTED`]. A
/// missing line, or one with a different source, gives `false`.
pub(in super::super) fn local_only(text: &str) -> bool {
    ["passwd", "group"].iter().all(|database| {
        let line = text
            .lines()
            .map(|line| line.split('#').next().unwrap_or_default().trim())
            .find_map(|line| line.strip_prefix(database)?.trim_start().strip_prefix(':'));
        line.is_some_and(|sources| {
            let mut sources = sources
                .split_whitespace()
                .filter(|word| !(word.starts_with('[') && word.ends_with(']')));
            sources.clone().next().is_some() && sources.all(|source| LISTED.contains(&source))
        })
    })
}

/// From the lines of `getent group`: the names in the list of members of each entry with the
/// number `group` (a number can have more than one entry). `None` when a line is not a group.
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

/// From the lines of `getent passwd`: each account whose own group is `group`. `None` when a line
/// is not an account.
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

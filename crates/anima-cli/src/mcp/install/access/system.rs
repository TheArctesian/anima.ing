//! The members of a group as this computer's account database lists them (R-NEW-exactgroup): the
//! names of the group's list of members, and each account whose own (primary) group it is. Linux
//! lists the accounts with `getent passwd`; macOS with `dscl`, which also shows nested groups,
//! members known only by identifier and groups whose members the system calculates (as
//! `everyone`): such a group counts as unread.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use nix::unistd::{Gid, Group, Uid, User};

use super::super::limited;
use super::{Groups, Listing, Member};

/// The longest time a program of the system may take to list the accounts.
const LIMIT: Duration = Duration::from_secs(10);

/// The most that such a program may write; more counts as unread.
const MOST: u64 = 16 << 20;

/// The directory program of macOS.
const DSCL: &str = "/usr/bin/dscl";

/// The groups of macOS whose members the system calculates and does not list: `everyone`,
/// `authedusers`, `interactusers`, `netusers`, `consoleusers`, `localaccounts`, `netaccounts`.
const CALCULATED: [u32; 7] = [12, 50, 51, 52, 53, 61, 62];

/// Where Linux systems keep `getent`; the last is NixOS.
const GETENT: [&str; 3] = [
    "/usr/bin/getent",
    "/bin/getent",
    "/run/current-system/sw/bin/getent",
];

/// The group with the number `group` and its members; the members are `None` when one step of
/// the reading fails.
pub(super) fn listing(group: u32) -> Listing {
    let found = Group::from_gid(Gid::from_raw(group)).ok().flatten();
    Listing {
        name: found
            .as_ref()
            .map_or_else(|| group.to_string(), |found| found.name.clone()),
        members: found.as_ref().and_then(members),
    }
}

/// The name of the user with the number `user`, or the number when it has no name.
pub(super) fn account(user: u32) -> String {
    User::from_uid(Uid::from_raw(user))
        .ok()
        .flatten()
        .map_or_else(|| user.to_string(), |found| found.name)
}

fn members(group: &Group) -> Option<Vec<Member>> {
    let number = group.gid.as_raw();
    let mut names = group.mem.clone();
    let mut members = Vec::new();
    if cfg!(target_os = "macos") {
        if CALCULATED.contains(&number) {
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
        names.extend(record(&ask(Path::new(DSCL), &arguments)?, number)?);
        let users = ask(
            Path::new(DSCL),
            &["/Search", "-list", "/Users", "PrimaryGroupID"],
        )?;
        names.extend(primary(&users, number)?);
    } else {
        let getent = GETENT
            .into_iter()
            .map(Path::new)
            .find(|path| path.exists())?;
        members.extend(passwd(&ask(getent, &["passwd"])?, number)?);
    }
    for name in names {
        let uid = User::from_name(&name).ok()?.map(|user| user.uid.as_raw());
        members.push(Member { name, uid });
    }
    Some(members)
}

/// A group name that can be a part of a `dscl` path.
fn plain(name: &str) -> Option<&str> {
    (!name.is_empty() && !name.starts_with('-') && !name.contains('/')).then_some(name)
}

/// What the system program at `program` writes for `arguments`: only a program that no user but
/// root can change, that ends without failure within [`LIMIT`] and writes at most [`MOST`] bytes
/// of UTF-8 text.
fn ask(program: &Path, arguments: &[&str]) -> Option<String> {
    // Its groups are not read, as that would ask again: each group that can write it counts.
    let unread = Groups::new(|group| Listing {
        name: group.to_string(),
        members: None,
    });
    if super::replaceable(program, 0, &unread).is_some() {
        return None;
    }
    let mut command = Command::new(program);
    command
        .args(arguments)
        .env_clear()
        .env("LC_ALL", "C")
        .current_dir("/");
    let output = limited::run(command, LIMIT, MOST)?;
    if !output.success || output.stdout.len() as u64 > MOST {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// From the lines of `getent passwd`: each account whose own group is `group`. `None` when a line
/// is not an account.
pub(super) fn passwd(text: &str, group: u32) -> Option<Vec<Member>> {
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

/// From `dscl -list /Users PrimaryGroupID`: the name of each account whose own group is `group`.
/// `None` when a line has more than a name and one number.
pub(super) fn primary(text: &str, group: u32) -> Option<Vec<String>> {
    let mut names = Vec::new();
    for line in text.lines() {
        match line.split_whitespace().collect::<Vec<_>>().as_slice() {
            // A record with no own group.
            [] | [_] => {}
            [name, own] => {
                if number(own)? == group {
                    names.push((*name).to_owned());
                }
            }
            _ => return None,
        }
    }
    Some(names)
}

/// A number of an account or a group as the system writes it. `dscl` writes some as negative
/// numbers (`nobody` is -2): the same 32 bits as the number that the kernel uses (4294967294).
pub(super) fn number(text: &str) -> Option<u32> {
    let value: i64 = text.parse().ok()?;
    (i64::from(i32::MIN)..=i64::from(u32::MAX))
        .contains(&value)
        .then(|| u32::try_from(value.rem_euclid(1 << 32)).ok())
        .flatten()
}

/// From `dscl -read` of the group with the number `group`: the names of its list of members.
/// `None` when the record is of a different group, has no list of members (as a group whose
/// members the system calculates), has nested groups or more members by identifier than by name,
/// or has a line that is not a value.
pub(super) fn record(text: &str, group: u32) -> Option<Vec<String>> {
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
    let count = |name: &str| values.get(name).map_or(0, Vec::len);
    let numbers = values.get("PrimaryGroupID").map(Vec::as_slice);
    let same = matches!(numbers, Some([own]) if number(own) == Some(group));
    if !same || count("NestedGroups") > 0 || count("GroupMembers") > count("GroupMembership") {
        return None;
    }
    let members = values.remove("GroupMembership")?;
    Some(members.into_iter().map(str::to_owned).collect())
}

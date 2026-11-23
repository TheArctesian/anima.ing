//! The members of a group as this computer's own account database lists them (R-442):
//! the names of the group's list of members, and each account whose own (primary) group it is.
//! When the computer also takes accounts from a directory service, which need not list each of
//! them, no group counts as read.

pub(super) mod linux;
pub(super) mod macos;

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

/// The group with the number `group` and its members; the members are `None` when one step of
/// the reading fails.
pub(super) fn listing(group: u32) -> Listing {
    let found = Group::from_gid(Gid::from_raw(group)).ok().flatten();
    Listing {
        name: found
            .as_ref()
            .map_or_else(|| group.to_string(), |found| found.name.clone()),
        members: found.as_ref().and_then(|found| {
            if cfg!(target_os = "macos") {
                macos::members(found)
            } else {
                linux::members(found)
            }
        }),
    }
}

/// The name of the user with the number `user`, or the number when it has no name.
pub(super) fn account(user: u32) -> String {
    User::from_uid(Uid::from_raw(user))
        .ok()
        .flatten()
        .map_or_else(|| user.to_string(), |found| found.name)
}

/// The members with the names `names`, each with the number of its account; a name with no account
/// has none. `None` when the account database cannot be read.
fn by_name(names: impl IntoIterator<Item = String>) -> Option<Vec<Member>> {
    names
        .into_iter()
        .map(|name| {
            let uid = User::from_name(&name).ok()?.map(|user| user.uid.as_raw());
            Some(Member { name, uid })
        })
        .collect()
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

/// A number of an account or a group as the system writes it. `dscl` writes some as negative
/// numbers (`nobody` is -2): the same 32 bits as the number that the kernel uses (4294967294).
pub(super) fn number(text: &str) -> Option<u32> {
    let value: i64 = text.parse().ok()?;
    (i64::from(i32::MIN)..=i64::from(u32::MAX))
        .contains(&value)
        .then(|| u32::try_from(value.rem_euclid(1 << 32)).ok())
        .flatten()
}

#[cfg(test)]
mod tests;

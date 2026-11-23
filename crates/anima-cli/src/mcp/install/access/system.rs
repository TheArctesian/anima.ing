//! Group members from this computer's account database (R-442). With a directory service, which
//! need not list each account, no group counts as read.

pub(super) mod linux;
pub(super) mod macos;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use nix::unistd::{Gid, Group, Uid, User};

use super::super::limited;
use super::{Groups, Listing, Member};

const LIMIT: Duration = Duration::from_secs(10);

const MOST: u64 = 16 << 20;

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

pub(super) fn account(user: u32) -> String {
    User::from_uid(Uid::from_raw(user))
        .ok()
        .flatten()
        .map_or_else(|| user.to_string(), |found| found.name)
}

fn by_name(names: impl IntoIterator<Item = String>) -> Option<Vec<Member>> {
    names
        .into_iter()
        .map(|name| {
            let uid = User::from_name(&name).ok()?.map(|user| user.uid.as_raw());
            Some(Member { name, uid })
        })
        .collect()
}

/// Runs only a program that no user but root can change.
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

/// `dscl` writes some numbers as negative (`nobody` is -2): the same 32 bits the kernel uses.
pub(super) fn number(text: &str) -> Option<u32> {
    let value: i64 = text.parse().ok()?;
    (i64::from(i32::MIN)..=i64::from(u32::MAX))
        .contains(&value)
        .then(|| u32::try_from(value.rem_euclid(1 << 32)).ok())
        .flatten()
}

#[cfg(test)]
mod tests;

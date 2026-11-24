//! Whether a folder on the daemon binary's path can be changed only by root or the installing user (MET-019).
//! It copies the daemon's user-unit rule (`crates/daemon/src/units/trust.rs`), as the shell cannot depend on the daemon crate, and is never looser: the shell hands the code to the binary before the daemon's own check runs.
//! It is stricter for sticky folders that others or a shared group can write; `crates/daemon/tests/trust-cases.txt` pins both rules (rows `DIFF`).

/// With a POSIX access ACL the group bits of the mode are the ACL's mask, which named users and groups get too.
#[derive(Clone, Copy, Debug)]
pub struct Stat {
    pub mode: u32,
    pub owner: u32,
    pub group: u32,
    pub acl: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct User {
    pub uid: u32,
    pub private_group: Option<u32>,
}

struct Account<'a> {
    name: &'a str,
    uid: u32,
    gid: u32,
}

fn account(line: &str) -> Option<Account<'_>> {
    let mut fields = line.split(':');
    let name = fields.next()?;
    let uid = fields.nth(1)?.trim().parse().ok()?;
    let gid = fields.next()?.trim().parse().ok()?;
    Some(Account { name, uid, gid })
}

fn accounts(passwd: &str) -> impl Iterator<Item = Account<'_>> {
    passwd.lines().filter_map(account)
}

/// The primary group of `uid`, named as the user, with no other member and no other account in it; any line that does not read gives `None` (fails closed).
/// Only `/etc/passwd` and `/etc/group` are read: NSS sources and `/etc/gshadow` are unseen.
pub fn private_group(uid: u32, passwd: Option<&str>, group: Option<&str>) -> Option<u32> {
    let (passwd, group) = (passwd?, group?);
    if passwd
        .lines()
        .any(|line| !line.is_empty() && account(line).is_none())
    {
        return None;
    }
    let user = accounts(passwd).find(|account| account.uid == uid)?;
    if accounts(passwd).any(|account| account.gid == user.gid && account.uid != uid) {
        return None;
    }
    let mut found = false;
    for line in group.lines().filter(|line| !line.is_empty()) {
        let mut fields = line.split(':');
        let (Some(name), Some(gid), members) = (fields.next(), fields.nth(1), fields.next()) else {
            return None;
        };
        let Ok(gid) = gid.trim().parse::<u32>() else {
            return None;
        };
        if gid != user.gid {
            continue;
        }
        let only_user = members
            .unwrap_or("")
            .split(',')
            .all(|member| member.is_empty() || member == user.name);
        if name != user.name || !only_user {
            return None;
        }
        found = true;
    }
    found.then_some(user.gid)
}

fn group_write_safe(stat: Stat, by: User) -> bool {
    stat.mode & 0o020 == 0
        || (stat.owner == by.uid && by.private_group == Some(stat.group) && !stat.acl)
}

/// Whether a folder may hold, or lie above, the binary the user's unit starts at each login.
pub fn folder_trusted(stat: Stat, by: User) -> bool {
    (stat.owner == by.uid || stat.owner == 0)
        && stat.mode & 0o002 == 0
        && group_write_safe(stat, by)
}

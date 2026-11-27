//! Confirm that the source's observed systemd units cannot start at the next boot.
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

use nix::unistd::User;

use super::{Error, query};

pub(super) fn require_fenced(
    user: &User,
    units: &BTreeSet<String>,
    deadline: Instant,
) -> Result<(), Error> {
    if units.is_empty() {
        return Ok(());
    }
    if !units.iter().all(|name| service_name(name)) {
        return Err(Error::SourceLauncher);
    }
    let scope = if user.uid.is_root() {
        "--system"
    } else {
        "--user"
    };
    let mut args = vec![
        scope,
        "--no-pager",
        "--all",
        "--property=Id,LoadState,ActiveState,UnitFileState",
        "show",
        "--",
    ];
    args.extend(units.iter().map(String::as_str));
    let output = query(user, "systemctl", &args, deadline).map_err(|_| Error::SourceLauncher)?;
    verify(
        units,
        std::str::from_utf8(&output).map_err(|_| Error::SourceLauncher)?,
    )
}

pub(super) fn service_name(name: &str) -> bool {
    name.len() <= 255
        && name
            .strip_suffix(".service")
            .is_some_and(|stem| !stem.is_empty())
        && name.as_bytes().first().is_some_and(|byte| *byte != b'-')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.:@-\\".contains(&byte))
}

fn verify(units: &BTreeSet<String>, reply: &str) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for record in reply.trim_end().split("\n\n") {
        let values: BTreeMap<_, _> = record
            .lines()
            .filter_map(|line| line.split_once('='))
            .collect();
        let id = values.get("Id").copied().ok_or(Error::SourceLauncher)?;
        if record.lines().count() != 4
            || values.len() != 4
            || !units.contains(id)
            || !seen.insert(id)
            || values.get("LoadState") != Some(&"masked")
            || values.get("ActiveState") != Some(&"inactive")
            || values.get("UnitFileState") != Some(&"masked")
        {
            return Err(Error::SourceLauncher);
        }
    }
    if seen.len() != units.len() {
        return Err(Error::SourceLauncher);
    }
    Ok(())
}

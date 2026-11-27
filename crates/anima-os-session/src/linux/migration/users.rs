use super::super::{Error, Result, command};
use nix::unistd::{Uid, User};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub(super) fn read(path: &Path) -> Result<String> {
    let file = fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::Refused("Migration inventory is not a regular file."));
    }
    let mut value = String::new();
    file.take(65537).read_to_string(&mut value)?;
    if value.len() > 65536 {
        return Err(Error::Refused("Migration inventory exceeded its limit."));
    }
    Ok(value)
}

pub(super) fn account(uid: u32) -> Result<User> {
    User::from_uid(Uid::from_raw(uid))
        .map_err(std::io::Error::from)?
        .filter(|user| user.dir.is_absolute() && !user.name.starts_with('-'))
        .ok_or(Error::Refused("Choose an existing local account."))
}

pub(super) fn storage(user: &User) -> PathBuf {
    if user.uid.is_root() {
        PathBuf::from("/var/lib/containers/storage")
    } else {
        user.dir.join(".local/share/containers/storage")
    }
}

pub fn list() -> Result<Value> {
    let passwd = read(Path::new("/etc/passwd"))?;
    let mut users = Vec::new();
    for line in passwd.lines() {
        let fields: Vec<_> = line.split(':').collect();
        let Some(uid) = fields.get(2).and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let user = account(uid)?;
        let detected = storage(&user).try_exists()?;
        if uid != 0 && !detected {
            continue;
        }
        if users.len() >= 256 {
            return Err(Error::Refused("Too many legacy runtime accounts."));
        }
        users.push(json!({"uid":uid,"user":user.name,"defaultStorageDetected":detected}));
    }
    Ok(json!({"accounts":users,"complete":false,
        "note":"Standard local storage only. Custom storage roots and external account directories require separate inventory."}))
}

pub(super) fn subordinate(user: &User, file: &str) -> Result<Vec<String>> {
    Ok(read(Path::new(file))?
        .lines()
        .filter(|line| {
            line.split(':')
                .next()
                .is_some_and(|name| name == user.name || name == user.uid.to_string())
        })
        .map(str::to_owned)
        .collect())
}

pub(super) async fn run(
    user: &User,
    program: &str,
    arguments: &[&str],
    seconds: u64,
) -> Result<String> {
    let home = format!(
        "HOME={}",
        user.dir
            .to_str()
            .ok_or(Error::Refused("The account home path is invalid."))?
    );
    let runtime = format!("XDG_RUNTIME_DIR=/run/user/{}", user.uid);
    let mut args = vec![
        "-u",
        user.name.as_str(),
        "--",
        "/run/current-system/sw/bin/env",
        "-i",
        "PATH=/run/current-system/sw/bin",
        "LC_ALL=C.UTF-8",
        home.as_str(),
    ];
    if !user.uid.is_root() {
        args.push(runtime.as_str());
    }
    args.push(program);
    args.extend_from_slice(arguments);
    command::run("/run/current-system/sw/bin/runuser", &args, None, seconds).await
}

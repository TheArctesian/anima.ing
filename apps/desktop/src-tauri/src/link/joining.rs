//! One join to a hub (2.6, F-7): the invite goes on standard input, and neither it nor what `join` says is logged (LOG-007).

use std::{
    ffi::OsString,
    net::{Ipv4Addr, TcpListener},
    path::Path,
    time::Duration,
};

use serde::Serialize;

use super::{LinkError, folder, folder::LinkFolders, printed};
use crate::carried::run::{ExchangeError, Invocation, exchange};

/// `join` gives up after 30 s by itself; this is the shell's own limit.
const JOIN_LIMIT: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Joined {
    pub fingerprint: String,
    pub port: u16,
}

/// The folder is removed when the join fails.
pub fn join(
    program: &Path,
    folders: &LinkFolders,
    invite: &str,
    env: &[(&'static str, OsString)],
) -> Result<Joined, LinkError> {
    folders.prepare_next().map_err(|_| LinkError::Folder)?;
    let joined = join_into(program, &folders.next, invite, env);
    if joined.is_err() {
        let _ = folders.prepare_next();
    }
    joined
}

fn join_into(
    program: &Path,
    next: &Path,
    invite: &str,
    env: &[(&'static str, OsString)],
) -> Result<Joined, LinkError> {
    let (verb, state, next_path) = (
        OsString::from("join"),
        OsString::from("-state"),
        next.as_os_str().to_owned(),
    );
    let invocation = Invocation {
        program,
        args: &[&verb, &state, &next_path],
        env,
        limit: JOIN_LIMIT,
    };
    let printed = exchange(&invocation, Some(invite.as_bytes())).map_err(|error| match error {
        ExchangeError::Exited { said, .. } => printed::join_failure(&said.text()),
        _ => LinkError::Failed,
    })?;
    let fingerprint =
        printed::fingerprint(&String::from_utf8_lossy(&printed)).ok_or(LinkError::Failed)?;
    let port = free_port().ok_or(LinkError::Failed)?;
    folder::write_port(next, port).map_err(|_| LinkError::Folder)?;
    Ok(Joined { fingerprint, port })
}

fn free_port() -> Option<u16> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
    listener.local_addr().ok().map(|address| address.port())
}

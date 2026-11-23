//! The first cgroup inspection waits until the launcher has become the runtime.

use std::io;
use std::os::{fd::OwnedFd, unix::net::UnixStream};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt as _;
use tokio::process::Command;

pub(super) fn attach(command: &mut Command) -> io::Result<tokio::net::UnixStream> {
    let (parent, child) = UnixStream::pair()?;
    parent.set_nonblocking(true)?;
    command.stdin(Stdio::from(OwnedFd::from(child)));
    tokio::net::UnixStream::from_std(parent)
}

pub(super) async fn wait(mut stream: tokio::net::UnixStream) -> io::Result<()> {
    let mut bytes = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(5),
        (&mut stream).take(2).read_to_end(&mut bytes),
    )
    .await
    .map_err(|_| io::Error::other("the launcher did not finish containment"))??;
    if bytes != [0] {
        return Err(io::Error::other("the launcher did not become the runtime"));
    }
    Ok(())
}

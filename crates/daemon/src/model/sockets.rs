//! The socket table of a server's tree (Section 9), read from the system, never by connecting.
//! On Linux each member's network namespace is read, so a member in its own namespace hides no listener.
//! On macOS `lsof` reads it, as the workspace forbids the `unsafe` that `proc_pidinfo` needs (task L1-6).

use std::io;

use daemon_core::model_work::loopback::Socket;

use super::tree::Identity;

/// # Errors
/// An I/O error when the table cannot be read or `members` is empty; the caller then treats the server as exposed.
pub async fn table(members: &[Identity]) -> io::Result<Vec<Socket>> {
    if members.is_empty() {
        return Err(io::Error::other("the tree of a running server is empty"));
    }
    #[cfg(target_os = "linux")]
    {
        linux(members).await
    }
    #[cfg(not(target_os = "linux"))]
    {
        lsof(members).await
    }
}

#[cfg(not(target_os = "linux"))]
const LSOF_SECONDS: u64 = 5;

#[cfg(not(target_os = "linux"))]
async fn lsof(members: &[Identity]) -> io::Result<Vec<Socket>> {
    let pids: Vec<String> = members
        .iter()
        .map(|member| member.pid.to_string())
        .collect();
    let run = tokio::process::Command::new("/usr/sbin/lsof")
        .args(["-nP", "-a", "-p", &pids.join(","), "-i", "-F", "PnT"])
        .env_clear()
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(std::time::Duration::from_secs(LSOF_SECONDS), run)
        .await
        .map_err(|_| io::Error::other("lsof did not end in 5 seconds"))??;
    // `lsof` ends with 1 when a selection matched nothing; its output is still the table.
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(io::Error::other("lsof did not read the socket table"));
    }
    Ok(daemon_core::model_work::loopback::read_lsof(
        &String::from_utf8_lossy(&output.stdout),
    ))
}

#[cfg(target_os = "linux")]
async fn linux(members: &[Identity]) -> io::Result<Vec<Socket>> {
    let inodes = socket_inodes(members).await?;
    let mut namespaces = std::collections::HashSet::new();
    let mut sockets = Vec::new();
    for member in members {
        let namespace = match tokio::fs::read_link(format!("/proc/{}/ns/net", member.pid)).await {
            Ok(namespace) => namespace,
            Err(_) if gone(member) => continue,
            Err(error) => return Err(error),
        };
        if namespaces.contains(&namespace) {
            continue;
        }
        match namespace_table(member.pid, &inodes).await {
            Ok(found) => {
                sockets.extend(found);
                namespaces.insert(namespace);
            }
            Err(_) if gone(member) => {}
            Err(error) => return Err(error),
        }
    }
    if namespaces.is_empty() {
        return Err(io::Error::other(
            "no member of the tree has a network namespace",
        ));
    }
    Ok(sockets)
}

/// A member that still runs but whose files do not read (not dumpable) is not gone, so the read fails closed.
#[cfg(target_os = "linux")]
fn gone(member: &Identity) -> bool {
    super::tree::start_of(member.pid).as_deref() != Some(member.start.as_str())
}

#[cfg(target_os = "linux")]
async fn socket_inodes(members: &[Identity]) -> io::Result<std::collections::HashSet<u64>> {
    let mut inodes = std::collections::HashSet::new();
    for member in members {
        let mut fds = match tokio::fs::read_dir(format!("/proc/{}/fd", member.pid)).await {
            Ok(fds) => fds,
            Err(_) if gone(member) => continue,
            Err(error) => return Err(error),
        };
        while let Ok(Some(fd)) = fds.next_entry().await {
            let Ok(target) = tokio::fs::read_link(fd.path()).await else {
                continue;
            };
            let target = target.to_string_lossy();
            if let Some(inode) = target
                .strip_prefix("socket:[")
                .and_then(|rest| rest.strip_suffix(']'))
            {
                inodes.extend(inode.parse::<u64>().ok());
            }
        }
    }
    Ok(inodes)
}

#[cfg(target_os = "linux")]
async fn namespace_table(
    pid: i32,
    inodes: &std::collections::HashSet<u64>,
) -> io::Result<Vec<Socket>> {
    use daemon_core::model_work::loopback::{Protocol, read_proc_net};
    let mut sockets = Vec::new();
    for (name, protocol) in [
        ("tcp", Protocol::Tcp),
        ("tcp6", Protocol::Tcp),
        ("udp", Protocol::Udp),
        ("udp6", Protocol::Udp),
    ] {
        let text = tokio::fs::read_to_string(format!("/proc/{pid}/net/{name}")).await?;
        sockets.extend(
            read_proc_net(&text, protocol)
                .into_iter()
                .filter(|(_, inode)| inodes.contains(inode))
                .map(|(socket, _)| socket),
        );
    }
    Ok(sockets)
}

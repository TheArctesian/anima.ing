//! Observe the destinations the workspace's IPv4 loopback preview connector can reach.
use super::command;
use agent_threads::managed::servers::{Listeners, MAX_PORTS};
use std::{collections::BTreeSet, net::Ipv4Addr, sync::Arc, time::Duration};
use tokio::{sync::Semaphore, task::JoinSet, time::Instant};
use uuid::Uuid;

pub(in crate::linux) async fn observe(containers: Vec<(Uuid, u64, String)>) -> Vec<Listeners> {
    let slots = Arc::new(Semaphore::new(4));
    let mut probes = JoinSet::new();
    for (workspace, generation, container) in containers {
        let slots = Arc::clone(&slots);
        probes.spawn(async move {
            let _slot = slots.acquire_owned().await.ok()?;
            let tables = command::docker(&[
                "exec",
                "--user",
                "1000:1000",
                &container,
                "/bin/cat",
                "/proc/net/tcp",
                "/proc/net/tcp6",
            ])
            .await
            .ok()?;
            let ports = listening(&tables);
            (!ports.is_empty()).then_some(Listeners {
                workspace,
                generation,
                ports,
            })
        });
    }
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut servers = Vec::new();
    while let Ok(Some(result)) = tokio::time::timeout_at(deadline, probes.join_next()).await {
        if let Ok(Some(ports)) = result {
            servers.push(ports);
        }
    }
    servers
}

fn listening(tables: &str) -> Vec<u16> {
    tables
        .lines()
        .filter_map(listener)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(MAX_PORTS)
        .collect()
}

fn listener(line: &str) -> Option<u16> {
    let mut fields = line.split_ascii_whitespace();
    let (address, port) = fields.nth(1)?.split_once(':')?;
    if fields.nth(1)? != "0A" || !reachable(address)? {
        return None;
    }
    u16::from_str_radix(port, 16).ok().filter(|port| *port != 0)
}

fn reachable(address: &str) -> Option<bool> {
    let word = |text: &str| u32::from_str_radix(text, 16).ok().map(u32::to_ne_bytes);
    let ipv4 = |bytes| Ipv4Addr::from(bytes);
    match address.len() {
        8 => Some(matches!(
            ipv4(word(address)?),
            Ipv4Addr::LOCALHOST | Ipv4Addr::UNSPECIFIED
        )),
        32 if address.is_ascii() => {
            let [a, b, c, d] = [
                word(&address[..8])?,
                word(&address[8..16])?,
                word(&address[16..24])?,
                word(&address[24..])?,
            ];
            Some(
                a == [0; 4]
                    && b == [0; 4]
                    && c == [0, 0, 255, 255]
                    && ipv4(d) == Ipv4Addr::LOCALHOST,
            )
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;

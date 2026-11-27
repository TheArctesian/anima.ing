//! Observe a started project after handing back the mutation lock.
use std::{
    os::unix::net::UnixStream,
    thread,
    time::{Duration, Instant},
};

use anima_applications_wire::{Health, Reply};
use compose_rules::Node;

use crate::{Paths, work::apply::Prepared};

use super::{
    observe::{self, ProjectContainer},
    wire,
};

pub(super) fn report(
    (paths, stream): (&Paths, &mut UnixStream),
    (prepared, project, folder): (Prepared, &str, &str),
    deadline: Instant,
    authorized: impl Fn() -> bool,
) -> Reply {
    if wire::send(stream, &Reply::CheckingHealth).is_err() {
        return Reply::Applied {
            health: Health::NotWaited,
        };
    }
    let services = services(&prepared);
    drop(prepared);
    let health = wait(paths, (project, folder, &services), deadline, || {
        authorized() && wire::connected(stream)
    });
    Reply::Applied { health }
}

fn services(prepared: &Prepared) -> Vec<String> {
    prepared
        .checked
        .document
        .get("services")
        .and_then(Node::entries)
        .unwrap_or_default()
        .iter()
        .map(|(name, _)| name.clone())
        .collect()
}

fn wait(
    paths: &Paths,
    (project, folder, services): (&str, &str, &[String]),
    deadline: Instant,
    continue_waiting: impl Fn() -> bool,
) -> Health {
    if services.is_empty() {
        return Health::NotWaited;
    }
    loop {
        if Instant::now() >= deadline || !continue_waiting() {
            return Health::NotWaited;
        }
        let Ok(containers) = observe::containers(paths, project, deadline) else {
            return Health::NotWaited;
        };
        let names = services.iter().map(String::as_str);
        if let Some(health) = state(&containers, folder, names) {
            return health;
        }
        thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_secs(1)),
        );
    }
}

pub(super) fn state<'a>(
    containers: &[ProjectContainer],
    folder: &str,
    services: impl Iterator<Item = &'a str>,
) -> Option<Health> {
    let mut checked = false;
    let mut waiting = false;
    for name in services {
        let mut targets = containers
            .iter()
            .filter(|one| one.service == name && one.folder == folder);
        let Some(first) = targets.next() else {
            return Some(Health::Unhealthy);
        };
        for container in std::iter::once(first).chain(targets) {
            if !container.running {
                return Some(Health::Unhealthy);
            }
            match container.health.as_deref() {
                None => (),
                Some("healthy") => checked = true,
                Some("starting") => waiting = true,
                Some("unhealthy") => return Some(Health::Unhealthy),
                Some(_) => return Some(Health::NotWaited),
            }
        }
    }
    if waiting {
        None
    } else if checked {
        Some(Health::Healthy)
    } else {
        Some(Health::NoCheck)
    }
}

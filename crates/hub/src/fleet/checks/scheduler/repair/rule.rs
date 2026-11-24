//! CHK-013, R-064. Pure.

use serde_json::{Value, json};

use crate::{
    fleet::{
        checks::{config::CheckConfig, store::repairs::Facts},
        devices::Device,
    },
    status::Status,
};

pub(super) const TIMEOUT_MS: u32 = 5000;
/// The database's limit on a check name, in characters.
const NAME_MAX: usize = 200;

/// An inverted check that is Down did get replies, so its method works.
pub(super) fn is_candidate(facts: &Facts, in_maintenance: bool) -> bool {
    matches!(facts.config, Some(CheckConfig::Icmp { .. }))
        && !facts.settings.invert
        && facts.status == Status::Down
        && facts.failures > u32::from(facts.settings.retries)
        && facts.answered == Some(false)
        && !facts.operator_edited
        && facts.tried_at.is_none()
        && !facts.paused
        && !in_maintenance
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Protocol {
    Ssh,
}

impl Protocol {
    const fn label(self) -> &'static str {
        match self {
            Self::Ssh => "SSH",
        }
    }

    const fn usual_port(self) -> u16 {
        match self {
            Self::Ssh => 22,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Port {
    pub(super) protocol: Protocol,
    pub(super) number: u16,
}

impl Port {
    pub(super) const fn config(self) -> CheckConfig {
        CheckConfig::Tcp {
            port: self.number,
            timeout_ms: TIMEOUT_MS,
        }
    }

    fn check_name(self) -> String {
        let label = self.protocol.label();
        if self.number == self.protocol.usual_port() {
            format!("{label} port")
        } else {
            format!("{label} port {}", self.number)
        }
    }
}

pub(super) fn ports(device: &Device) -> Vec<Port> {
    [device.ssh.enabled.then_some(Port {
        protocol: Protocol::Ssh,
        number: device.ssh.port,
    })]
    .into_iter()
    .flatten()
    .collect()
}

fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// A number follows when both names are taken, so the write never fails on the unique name
/// and retries forever (D-5).
pub(super) fn repaired_name(port: Port, old: &str, taken: &[String]) -> String {
    let free = |name: &str| !taken.iter().any(|other| other == name);
    let base = port.check_name();
    if free(&base) {
        return base;
    }
    let was = cut(&format!("{base} (was {old})"), NAME_MAX);
    if free(&was) {
        return was;
    }
    let mut number = 2_usize;
    loop {
        let suffix = format!(" {number}");
        let name = format!("{}{suffix}", cut(&was, NAME_MAX - suffix.len()));
        if free(&name) {
            return name;
        }
        number += 1;
    }
}

/// Device, protocol and port only (LOG-007).
pub(super) fn repaired_message(device: &str, port: Port) -> String {
    format!(
        "Ping never answered on {device}; {} port {} does, so its check now uses TCP {}.",
        port.protocol.label(),
        port.number,
        port.number
    )
}

pub(super) fn silent_message(device: &str) -> String {
    format!("Ping never answered on {device}, and no connection port answers either.")
}

/// ICMP and TCP configurations hold no secret.
fn method(name: &str, config: &CheckConfig) -> Value {
    json!({
        "type": config.kind().as_sql(),
        "config": serde_json::to_value(config).unwrap_or(Value::Null),
        "name": name,
    })
}

/// The audit change of a repair (D-4).
pub(super) fn audit_change(before: (&str, &CheckConfig), after: (&str, &CheckConfig)) -> Value {
    json!({
        "repairedBy": "hub",
        "before": method(before.0, before.1),
        "after": method(after.0, after.1),
    })
}

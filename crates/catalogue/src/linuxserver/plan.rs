//! The decisions of one image's recipe, before any text is written.

use compose_rules::forms::{is_name, is_target};

use super::{Config, Image, Unfit};
use crate::{ANIMA_PORTS, entry::Protocol, example::is_secret_name};

/// The shell images (Section 15.7 of the proposal) with their credential variables. The class
/// comes from this fixed list, never from the data of the publisher.
const SHELL: [(&str, &[&str]); 7] = [
    ("code-server", &["PASSWORD"]),
    ("openvscode-server", &["CONNECTION_TOKEN"]),
    ("transmission", &["USER", "PASS"]),
    ("webtop", &["PASSWORD"]),
    ("syncthing", &[]),
    ("openssh-server", &[]),
    ("rdesktop", &[]),
];
/// The images whose hardware decoding needs `/dev/dri`, which their data gives only in text.
pub(super) const HARDWARE_DECODING: [&str; 2] = ["jellyfin", "plex"];
const RENDER_DEVICE: &str = "/dev/dri";
/// `PUID` and `PGID`: the user of the init after the start, never root.
const NON_ROOT: &str = "1000";
const ZONE: &str = "Etc/UTC";
const LOWEST_PORT: u16 = 1024;
const PORT_STEP: u16 = 8000;
/// The variables of the init that get or install code at the start.
const MODS: [&str; 3] = ["DOCKER_MODS", "INSTALL_PACKAGES", "INSTALL_PIP_PACKAGES"];
const PLACEHOLDER: &str = "/path/to/";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Value {
    /// Written in the Compose file: `PUID`, `PGID`, `VERSION`.
    Fixed(&'static str),
    /// A variable of the environment file, with the example value of the publisher.
    Example(String),
    /// A variable that the install makes: a secret or a credential.
    Generated,
}

#[derive(Clone, Debug)]
pub(super) struct Setting {
    pub name: String,
    pub value: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Published {
    pub target: u16,
    pub on_machine: u16,
    pub protocol: Protocol,
}

#[derive(Clone, Debug)]
pub(super) struct Plan {
    /// The name of the service, which is also the identifier of the entry.
    pub service: String,
    pub settings: Vec<Setting>,
    /// Each named volume and its path in the container.
    pub volumes: Vec<(String, String)>,
    pub ports: Vec<Published>,
    /// The devices of the optional parts, which are off.
    pub devices: Vec<String>,
    pub shell: bool,
}

fn refusal(config: &Config) -> Option<Unfit> {
    let required = |optional: bool| !optional;
    if config.privileged {
        return Some(Unfit::Privileged);
    }
    if config.networking.as_deref() == Some("host") {
        return Some(Unfit::HostNetwork);
    }
    if let Some(capability) = config.caps.iter().find(|cap| required(cap.optional)) {
        return Some(Unfit::Capability(capability.cap_add.clone()));
    }
    if config
        .security_opt
        .iter()
        .any(|option| required(option.optional))
    {
        return Some(Unfit::SecurityOption);
    }
    if config
        .devices
        .iter()
        .any(|device| required(device.optional))
    {
        return Some(Unfit::Device);
    }
    for custom in config
        .custom
        .iter()
        .filter(|custom| required(custom.optional))
    {
        match custom.name_compose.as_str() {
            "shm_size" => return Some(Unfit::SharedMemory),
            "sysctls" => return Some(Unfit::KernelSetting),
            _ => {}
        }
    }
    let variables = config.env_vars.iter().filter(|var| required(var.optional));
    for variable in variables {
        if MODS.contains(&variable.name.as_str()) {
            return Some(Unfit::ImageMods);
        }
        if variable.name == "VERSION" && variable.value != "docker" {
            return Some(Unfit::VersionNotFixed);
        }
    }
    config
        .volumes
        .iter()
        .filter(|volume| required(volume.optional) && !is_placeholder(&volume.host_path))
        .map(|volume| {
            if volume.host_path.contains("docker.sock") {
                Unfit::EngineSocket
            } else {
                Unfit::HostPath
            }
        })
        .next()
}

fn is_placeholder(host_path: &str) -> bool {
    host_path.is_empty() || host_path.starts_with(PLACEHOLDER)
}

fn named(text: &str) -> String {
    text.chars()
        .map(|character| match character.to_ascii_lowercase() {
            lower @ ('a'..='z' | '0'..='9' | '-' | '_') => lower,
            _ => '-',
        })
        .collect()
}

fn is_variable(name: &str) -> bool {
    name.bytes()
        .next()
        .is_some_and(|byte| !byte.is_ascii_digit())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn setting(name: &str, value: Value) -> Setting {
    Setting {
        name: name.to_owned(),
        value,
    }
}

fn settings(config: &Config, credential: &[&str]) -> Result<Vec<Setting>, Unfit> {
    let mut found = vec![
        setting("PUID", Value::Fixed(NON_ROOT)),
        setting("PGID", Value::Fixed(NON_ROOT)),
        setting("TZ", Value::Example(ZONE.to_owned())),
    ];
    let required = config
        .env_vars
        .iter()
        .filter(|var| !var.optional)
        .map(|var| (var.name.as_str(), var.value.as_str()));
    let made = credential.iter().map(|name| (*name, ""));
    for (name, value) in required.chain(made) {
        if found.iter().any(|known| known.name == name) {
            continue;
        }
        if !is_variable(name) || value.chars().any(char::is_control) {
            return Err(Unfit::VariableNotPermitted);
        }
        let value = if name == "VERSION" {
            Value::Fixed("docker")
        } else if is_secret_name(name) || credential.contains(&name) {
            Value::Generated
        } else if value.is_empty() {
            return Err(Unfit::ValueNeeded);
        } else {
            Value::Example(value.to_owned())
        };
        found.push(setting(name, value));
    }
    Ok(found)
}

fn volumes(config: &Config) -> Result<Vec<(String, String)>, Unfit> {
    let mut found: Vec<(String, String)> = Vec::new();
    for volume in config
        .volumes
        .iter()
        .filter(|volume| is_placeholder(&volume.host_path))
    {
        let name = named(volume.path.trim_matches('/')).replace('/', "-");
        let known = found.iter().any(|(known, _)| *known == name);
        if !is_name(&name) || !is_target(&volume.path) || known {
            return Err(Unfit::VolumeNotPermitted);
        }
        found.push((name, volume.path.clone()));
    }
    Ok(found)
}

/// A port of Anima (SF-252) or of `taken` moves to the next free port.
fn on_machine(external: u16, taken: &[u16]) -> Option<u16> {
    let mut moved = if external < LOWEST_PORT {
        external + PORT_STEP
    } else {
        external
    };
    while ANIMA_PORTS.contains(&moved) || taken.contains(&moved) {
        moved = moved.checked_add(1)?;
    }
    Some(moved)
}

/// The name reaches the `image:` line and a comment of the Compose file (SF-252).
fn is_image_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn ports(config: &Config, taken: &[u16]) -> Result<Vec<Published>, Unfit> {
    let mut found: Vec<Published> = Vec::new();
    for port in config.ports.iter().filter(|port| !port.optional) {
        let (target, protocol) = match port.internal.split_once('/') {
            Some((target, "udp")) => (target, Protocol::Udp),
            Some((target, "tcp")) => (target, Protocol::Tcp),
            None => (port.internal.as_str(), Protocol::Tcp),
            Some(_) => return Err(Unfit::PortNotPermitted),
        };
        let number = |text: &str| text.parse::<u16>().ok().filter(|port| *port > 0);
        let target = number(target).ok_or(Unfit::PortNotPermitted)?;
        let external = number(&port.external).ok_or(Unfit::PortNotPermitted)?;
        let on_machine = on_machine(external, taken).ok_or(Unfit::PortNotPermitted)?;
        let next = Published {
            target,
            on_machine,
            protocol,
        };
        if found
            .iter()
            .any(|known| known.on_machine == on_machine && known.protocol == protocol)
        {
            return Err(Unfit::PortNotPermitted);
        }
        found.push(next);
    }
    Ok(found)
}

pub(super) fn plan(image: &Image, taken: &[u16]) -> Result<Plan, Unfit> {
    let config = &image.config;
    if config.env_vars.is_empty() && config.volumes.is_empty() && config.ports.is_empty() {
        return Err(Unfit::NoConfig);
    }
    if let Some(reason) = refusal(config) {
        return Err(reason);
    }
    let service = named(&image.name);
    if !is_image_name(&image.name) || !is_name(&service) {
        return Err(Unfit::NameNotPermitted);
    }
    let shell = SHELL.iter().find(|(name, _)| *name == image.name);
    let credential = shell.map_or(&[][..], |(_, credential)| credential);
    if shell.is_some() && credential.is_empty() {
        return Err(Unfit::ShellWithoutCredential);
    }
    let ports = ports(config, taken)?;
    if !ports.iter().any(|port| port.protocol == Protocol::Tcp) {
        return Err(Unfit::NoPort);
    }
    let mut devices: Vec<String> = config
        .devices
        .iter()
        .filter(|device| device.optional && device.path.starts_with("/dev/"))
        .map(|device| device.path.clone())
        .collect();
    if HARDWARE_DECODING.contains(&image.name.as_str())
        && !devices.iter().any(|d| d == RENDER_DEVICE)
    {
        devices.push(RENDER_DEVICE.to_owned());
    }
    Ok(Plan {
        service,
        settings: settings(config, credential)?,
        volumes: volumes(config)?,
        ports,
        devices,
        shell: shell.is_some(),
    })
}

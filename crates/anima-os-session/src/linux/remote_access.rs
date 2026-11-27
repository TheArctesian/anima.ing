//! Managed defaults never overwrite a pre-existing link access decision.
use super::{Error, Result, command};
mod policy;

use policy::{DEFAULTS, Origin, Storage};
use remote_login::{LocalProtocol, Observation, observe};
use serde::Deserialize;
use serde_json::{Value, json};

pub fn initialize() -> Result<()> {
    let _lock = command::lock("/run/anima-remote-access.lock")?;
    let user = nix::unistd::User::from_name("anima-daemon")
        .map_err(std::io::Error::from)?
        .ok_or(Error::Refused("The private link account is unavailable."))?;
    Storage::live().initialize((user.uid, user.gid))
}

async fn status() -> Result<Value> {
    let storage = Storage::live();
    let selected = storage.ports()?;
    let policy = storage.policy()?;
    let (ssh, rdp, vnc) = tokio::join!(
        observe(LocalProtocol::Ssh),
        observe(LocalProtocol::Rdp),
        observe(LocalProtocol::Vnc),
    );
    let protocols = [
        ("SSH", LocalProtocol::Ssh, ssh),
        ("RDP", LocalProtocol::Rdp, rdp),
        ("VNC", LocalProtocol::Vnc, vnc),
    ].map(|(name, protocol, local)| {
        let port = protocol.port();
        let observed = match local {
            Observation::Unavailable => "unavailable",
            Observation::NoReply => "no-reply",
            Observation::Unexpected => "unexpected",
            Observation::Responding => "responding",
        };
        json!({"name":name,"port":port,"enabled":selected.as_ref().is_some_and(|ports| ports.contains(&port)),
            "listening":local != Observation::Unavailable,"localCheck":observed})
    });
    Ok(json!({
        "schema":1, "origin":policy.as_ref().map_or(json!("unmanaged"), |policy| json!(policy.origin)),
        "changed":policy.as_ref().is_some_and(|policy| selected.as_ref() != Some(&policy.baseline)),
        "configured":selected.is_some(), "protocols":protocols,
        "note":"Open a session from an Anima app to verify private-link access and sign-in."
    }))
}

async fn set(args: Value) -> Result<Value> {
    let port = args["port"]
        .as_u64()
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| DEFAULTS.contains(port))
        .ok_or(Error::Refused("Choose SSH, RDP or VNC."))?;
    let enabled = args["enabled"]
        .as_bool()
        .ok_or(Error::Refused("Choose whether the protocol is enabled."))?;
    if args["confirmed"] != true {
        return Err(Error::Refused("Confirm the private remote access change."));
    }
    let _lock = command::lock("/run/anima-remote-access.lock")?;
    command::run(
        "runuser",
        &[
            "-u",
            "anima-daemon",
            "--",
            "/run/current-system/sw/bin/anima-link",
            "ports",
            "-state",
            "/var/lib/anima-link",
            if enabled { "on" } else { "off" },
            &port.to_string(),
        ],
        None,
        10,
    )
    .await?;
    Storage::live().record_owner_choice()?;
    status().await
}

async fn retry_defaults(args: Value) -> Result<Value> {
    if args != json!({"confirmed":true}) {
        return Err(Error::Refused("Confirm retrying private access setup."));
    }
    if !Storage::live()
        .policy()?
        .is_some_and(|policy| policy.origin == Origin::PendingDefaults)
    {
        return Err(Error::Refused(
            "Default setup is no longer pending. Check private access again.",
        ));
    }
    // The boot unit uses the same locked initializer and keeps any saved choices.
    command::run(
        "systemctl",
        &["start", "anima-remote-access.service", "anima-link.service"],
        None,
        30,
    )
    .await
    .map_err(|_| {
        Error::Refused("Private access setup could not finish. Check the local service status.")
    })?;
    status().await
}

/// Disconnect graphical transports without restarting the shared console.
async fn disconnect(args: Value) -> Result<Value> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        protocol: String,
        confirmed: bool,
    }
    let request: Request = serde_json::from_value(args)?;
    if !request.confirmed {
        return Err(Error::Refused(
            "Confirm disconnecting current graphical clients.",
        ));
    }
    let unit = match request.protocol.as_str() {
        "RDP" => "xrdp.service",
        "VNC" => "anima-vnc.service",
        _ => return Err(Error::Refused("Choose RDP or VNC.")),
    };
    let _lock = command::lock("/run/anima-remote-access.lock")?;
    command::run("systemctl", &["restart", unit], None, 30).await?;
    Ok(
        json!({"protocol":request.protocol,"transportRestarted":true,"newConnections":"unchanged","sharedConsoleRestarted":false}),
    )
}

pub async fn dispatch(operation: &str, args: Value) -> Result<Value> {
    match operation {
        "remote-access-status" => status().await,
        "remote-disconnect" => disconnect(args).await,
        "remote-access-set" => set(args).await,
        "remote-access-retry-defaults" => retry_defaults(args).await,
        _ => Err(Error::Refused("Unsupported remote-access operation.")),
    }
}

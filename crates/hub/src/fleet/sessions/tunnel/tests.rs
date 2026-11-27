mod arguments;
mod client;
mod ending;
mod gateway;
mod join_record;
mod policy;
mod rdp_vnc_args;
mod size;
mod wire;
mod withheld;

use chrono::DateTime;
use guac_protocol::{Decoder, Instruction};
use uuid::Uuid;

use super::policy::Policy;
use super::relay::Relay;
use crate::fleet::devices::{
    Device, MetricsSource, OperatingSystem, RdpSecurity, RdpSettings, ResizeMethod, SshAuth,
    SshSettings, TransferPolicy, VncSettings,
};
use crate::fleet::sessions::Desktop;
use crate::ids::{DeviceId, SiteId};

const DEVICE_ID: &str = "0190a000-0000-7000-8002-0000000000aa";

/// Every RDP switch is on, so each value K4 Section 3 sets is set.
fn device(host: &str) -> Device {
    Device {
        id: DEVICE_ID.parse::<DeviceId>().unwrap(),
        site: SiteId::from(Uuid::nil()),
        name: "desk".to_owned(),
        host: host.to_owned(),
        ip: None,
        kind: "workstation".to_owned(),
        os: OperatingSystem::Windows,
        tags: Vec::new(),
        maintenance: false,
        ssh: SshSettings {
            enabled: false,
            port: 22,
            username: String::new(),
            auth: SshAuth::Password,
        },
        rdp: RdpSettings {
            enabled: true,
            port: 3390,
            username: "ops".to_owned(),
            domain: Some("LAB".to_owned()),
            security: RdpSecurity::NlaExt,
            ignore_certificate: true,
            resize: ResizeMethod::Reconnect,
            audio_out: true,
            microphone: true,
            shared_drive: true,
        },
        vnc: VncSettings {
            enabled: true,
            port: 5901,
            username: Some("viewer".to_owned()),
        },
        mac_address: None,
        metrics_source: MetricsSource::default(),
        distribution: None,
        session_limit: None,
        transfer: TransferPolicy::default(),
        created_at: DateTime::UNIX_EPOCH,
        updated_at: DateTime::UNIX_EPOCH,
    }
}

fn decoded(wire: &str) -> Instruction {
    let mut instructions = Decoder::new().feed(wire.as_bytes()).unwrap();
    assert_eq!(instructions.len(), 1, "one instruction in {wire:?}");
    instructions.remove(0)
}

fn relay_for(desktop: Desktop, edit: impl FnOnce(&mut Device)) -> Relay {
    let mut device = device("desk.lab");
    edit(&mut device);
    Relay::new(Policy::of(desktop, &device, false))
}

/// The database's defaults: no microphone and no shared drive.
fn relay() -> Relay {
    relay_for(Desktop::Rdp, |device| {
        device.rdp.microphone = false;
        device.rdp.shared_drive = false;
    })
}

fn prompted(names: &[&str]) -> Relay {
    let mut relay = relay();
    relay.on_gateway(Instruction::new("required", names.iter().copied()));
    relay
}

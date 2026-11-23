//! The tunnel's opening before the WebSocket upgrade (R-032, K4 Sections 1 to 3).
//! The hub reads the RDP certificate itself before guacd and the password, as guacd reports a mismatch only generically (SEC-007).
//! With `ANY`, guacd gets the TLS security the server chose, so it cannot fall back to standard RDP security.
//! The read is not kept: only a connection test lets the operator accept a certificate.

use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use guac_protocol::ConnectParams;
use tokio::time::timeout;

mod not_opened;

pub(super) use not_opened::NotOpened;

use super::{
    arguments::{Sources, audio, connect_values, pinned_security, time_zone},
    guacd::{self, Connected},
    policy::Policy,
    route::Route,
};
use crate::{
    fingerprint::CertificateFingerprint,
    fleet::{
        credentials::{self, Secret},
        devices,
        devices::{Device, RdpSecurity},
        sessions::{
            Desktop, PendingTicket,
            connection_test::{ReadError, Reply, TLS_PROTOCOLS, read_server_certificate},
            start::{certificate_required, protocol_off},
        },
        ssh::{self, SshError, Target},
        targets::{self, AddressPolicy, LinkPort, TargetError},
        trust::{self, ServerCertificate},
    },
};

pub(super) enum Opening {
    Gateway(Box<Opened>),
    CertificateChanged,
}

/// The policy is read with the device record, so it holds for the session's life (K4 Section 4).
pub(super) struct Opened {
    pub(super) connected: Connected,
    pub(super) policy: Policy,
    /// Carries guacd to a computer on the link for the session's life (HMD-048).
    pub(super) carried: Option<LinkPort>,
}

/// Safe to drop at any await: the password leaves the hub only in `connect`, after guacd's `args` (SEC-002).
pub(super) async fn open(route: &Route, ticket: &PendingTicket) -> Result<Opening, NotOpened> {
    let desktop = Desktop::of(ticket.protocol).ok_or(NotOpened::NotDesktop)?;
    let mut device = devices::get(&route.db, ticket.device)
        .await
        .map_err(|error| match error {
            devices::Error::NotFound(_) => NotOpened::DeviceGone,
            other => NotOpened::Devices(other),
        })?;
    if protocol_off(&device, ticket.protocol).is_some() {
        return Err(NotOpened::ProtocolOff);
    }
    let certificate = certificate(route, &device, desktop).await?;
    let policy = route.live.policy();
    let port = desktop.port(&device);
    let address = if ticket.wake_sent {
        let awake = awake(&device.host, port, policy, route.timing.reconnect);
        timeout(route.timing.woken_opening, awake)
            .await
            .map_err(|_| NotOpened::NotAwake)??
    } else {
        timeout(route.timing.connect, target(&device.host, port, policy))
            .await
            .map_err(|_| NotOpened::Unresolved)??
    };
    let pinned = certificate
        .as_ref()
        .filter(|_| certificate_required(&device.rdp));
    if let Some(accepted) = pinned {
        let Some(chosen) = presents(route, SocketAddr::new(address, port), accepted).await? else {
            return Ok(Opening::CertificateChanged);
        };
        device.rdp.security = pinned_security(device.rdp.security, chosen);
    }
    let checked = Checked {
        device: &device,
        address: SocketAddr::new(address, port),
        certificate: certificate.as_ref(),
    };
    through_gateway(route, ticket, desktop, checked).await
}

struct Checked<'a> {
    device: &'a Device,
    address: SocketAddr,
    certificate: Option<&'a ServerCertificate>,
}

/// guacd cannot reach a computer on the link, so it gets a hub port that carries there (HMD-048).
async fn through_gateway(
    route: &Route,
    ticket: &PendingTicket,
    desktop: Desktop,
    checked: Checked<'_>,
) -> Result<Opening, NotOpened> {
    let Checked {
        device,
        address: device_port,
        certificate,
    } = checked;
    let carried = LinkPort::open(route.live.policy(), device_port)
        .await
        .map_err(|_| NotOpened::Unreachable)?;
    let for_guacd = carried.as_ref().map_or(device_port, LinkPort::local);
    let password = password(route, device, desktop).await?;
    let sources = Sources {
        device,
        target: for_guacd.ip(),
        port: for_guacd.port(),
        password: password.as_ref(),
        certificate,
        drive_root: &route.drive_root,
    };
    let values = connect_values(desktop, &sources)?;
    drop(password);
    let params = ConnectParams {
        display: ticket.display,
        timezone: time_zone(ticket.timezone.as_deref()),
        name: ticket.operator.name.clone(),
        audio: audio(desktop, device),
        values,
    };
    let policy = Policy::of(desktop, device);
    let connected = guacd::connect(&route.guacd, desktop.select(), params, route.timing).await?;
    Ok(Opening::Gateway(Box::new(Opened {
        connected,
        policy,
        carried,
    })))
}

/// Opened only for the host the opening resolved; one entered for another host ends the opening (SEC-011).
async fn password(
    route: &Route,
    device: &Device,
    desktop: Desktop,
) -> Result<Option<Secret>, NotOpened> {
    let bound = (device.id, device.host.as_str());
    credentials::load_password(&route.db, &route.vault, bound, desktop.password())
        .await
        .map_err(|error| match error {
            credentials::Error::EnteredForOtherHost => NotOpened::CredentialMoved,
            other => NotOpened::Credentials(other),
        })
}

/// An RDP session that needs a certificate and has none is refused, as `start` refuses it (K4 Section 2).
async fn certificate(
    route: &Route,
    device: &Device,
    desktop: Desktop,
) -> Result<Option<ServerCertificate>, NotOpened> {
    if desktop == Desktop::Vnc {
        return Ok(None);
    }
    let accepted = trust::server_certificates(&route.db, &[device.id])
        .await
        .map_err(|error| NotOpened::Sessions(error.into()))?
        .remove(&device.id);
    if accepted.is_none() && certificate_required(&device.rdp) {
        return Err(NotOpened::NoCertificate);
    }
    Ok(accepted)
}

/// `None` when another certificate is presented; the address is an IP, so it is not resolved again.
async fn presents(
    route: &Route,
    address: SocketAddr,
    accepted: &ServerCertificate,
) -> Result<Option<RdpSecurity>, NotOpened> {
    let host = address.ip().to_string();
    let target = Target {
        host: &host,
        port: address.port(),
        policy: route.live.policy(),
    };
    let read = read_server_certificate(target, TLS_PROTOCOLS, route.timing.connect).await;
    match read.map_err(|error| match error {
        ReadError::Unresolved => NotOpened::Unresolved,
        ReadError::RefusedTarget => NotOpened::RefusedTarget,
        other => NotOpened::CertificateUnread(other),
    })? {
        Reply::Certificate(presented, chosen) => Ok(accepted
            .fingerprint_sha256
            .parse::<CertificateFingerprint>()
            .is_ok_and(|accepted| accepted == presented.fingerprint)
            .then_some(chosen)),
        Reply::Standard => Err(NotOpened::StandardOnly),
        Reply::Refused => Err(NotOpened::SecurityRefused),
    }
}

/// The first resolved address, when `policy` lets the gateway reach every one (R-030, R-032).
async fn target(host: &str, port: u16, policy: &AddressPolicy) -> Result<IpAddr, NotOpened> {
    let checked =
        targets::resolve_checked(host, port, policy)
            .await
            .map_err(|error| match error {
                TargetError::Refused => NotOpened::RefusedTarget,
                TargetError::Unresolved => NotOpened::Unresolved,
            })?;
    checked
        .first()
        .map(SocketAddr::ip)
        .ok_or(NotOpened::Unresolved)
}

/// The probe connection is closed at once; guacd makes its own.
async fn awake(
    host: &str,
    port: u16,
    policy: &AddressPolicy,
    retry: Duration,
) -> Result<IpAddr, NotOpened> {
    let stream = ssh::connect_waking(Target { host, port, policy }, retry)
        .await
        .map_err(|error| match error {
            SshError::RefusedTarget => NotOpened::RefusedTarget,
            _ => NotOpened::Unresolved,
        })?;
    // A computer on the link has no network address to wake; its link answers for it.
    let answered = stream.network_peer().ok_or(NotOpened::NotAwake)?;
    Ok(answered.to_canonical())
}

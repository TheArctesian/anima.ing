//! The devices that the hub adds by itself in the site of the link: its own device (HMD-045) and
//! a computer with no application (HM-T33).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use chrono::{DateTime, Utc};
use ipnet::{IpNet, Ipv6Net};
use sqlx::{Connection as _, PgConnection, PgExecutor};
use uuid::Uuid;

use super::{
    Error, FieldError,
    graphql::trail,
    model::{Device, DeviceChange, NewDeviceInput, NewSiteInput, OperatingSystem},
    remote::RemoteForm,
    rules::{validate_new_device, validate_new_site},
    store,
};
use crate::{
    fleet::events::{Bus, ClientAddr, Operator, Trail},
    ids::{DeviceId, SiteId},
    scalars::Cidr,
};

/// The hub's device is found by its host, so a name the operator gives it stays.
const HOST: &str = "localhost";
const KIND: &str = "hub";
const NAME: &str = "Hub";
const NAMES: usize = 10;
/// As the client makes it for the computers of the link (HMD-044).
const LINK_SITE: &str = "Link";
const LINK_SUBNET: Ipv6Net =
    Ipv6Net::new_assert(Ipv6Addr::new(0xfd41, 0x4e49, 0x4d41, 0, 0, 0, 0, 0), 48);

const NO_WAY_IN: &str = "No session reaches the container of the hub: its device takes no SSH, \
                         RDP or VNC.";

pub(super) fn is_hub_kind(kind: &str) -> bool {
    kind.trim().eq_ignore_ascii_case(KIND)
}

pub(super) fn kind_refused() -> FieldError {
    FieldError::new(
        "kind",
        "The kind hub is only for the hub's own device, and that device keeps it.",
    )
}

pub(super) fn refuse_new(kind: &str) -> Result<(), FieldError> {
    if is_hub_kind(kind) {
        Err(kind_refused())
    } else {
        Ok(())
    }
}

/// No device takes or leaves the kind `hub`, and the hub's device has no SSH, RDP or VNC
/// (HMD-045).
pub(super) fn refused_change(current: &str, change: &DeviceChange) -> Vec<FieldError> {
    let hub = is_hub_kind(current);
    let kind = change
        .kind
        .as_deref()
        .filter(|kind| is_hub_kind(kind) != hub);
    let ways = [
        ("ssh", change.ssh.as_ref().is_some_and(|ssh| ssh.enabled)),
        (
            "rdp",
            change.remote.rdp.as_ref().is_some_and(|rdp| rdp.enabled),
        ),
        (
            "vnc",
            change.remote.vnc.as_ref().is_some_and(|vnc| vnc.enabled),
        ),
    ];
    let ways = ways
        .into_iter()
        .filter(|&(_, enabled)| hub && enabled)
        .map(|(field, _)| FieldError::new(field, NO_WAY_IN));
    kind.map(|_| kind_refused())
        .into_iter()
        .chain(ways)
        .collect()
}

pub(super) async fn check_change<'e>(
    db: impl PgExecutor<'e>,
    id: DeviceId,
    change: &DeviceChange,
) -> Result<(), Error> {
    let remote = &change.remote;
    if change.kind.is_none() && change.ssh.is_none() && remote.rdp.is_none() && remote.vnc.is_none()
    {
        return Ok(());
    }
    let device = store::fetch(db, id).await?;
    let refused = refused_change(&device.kind, change);
    if refused.is_empty() {
        Ok(())
    } else {
        Err(Error::Validation(refused))
    }
}

pub(super) fn check_secret(device: &Device) -> Result<(), Error> {
    if is_hub_kind(&device.kind) {
        Err(Error::Validation(vec![FieldError::new(
            "deviceId", NO_WAY_IN,
        )]))
    } else {
        Ok(())
    }
}

pub(super) fn names() -> Vec<String> {
    let numbered = (2..NAMES).map(|number| format!("{NAME} (hosted {number})"));
    [NAME.to_owned(), format!("{NAME} (hosted)")]
        .into_iter()
        .chain(numbered)
        .collect()
}

pub(super) fn free_name(names: Vec<String>, taken: &[String]) -> Option<String> {
    names.into_iter().find(|name| !taken.contains(name))
}

/// The hub's own work is written for no operator (LOG-014).
fn hub_trail(bus: &Bus, client: ClientAddr, now: DateTime<Utc>) -> Trail<'_> {
    Trail::new(bus, (Operator::hub(), client), now)
}

const HUB_CLIENT: ClientAddr = ClientAddr(IpAddr::V4(Ipv4Addr::LOCALHOST));

async fn link_site(
    conn: &mut PgConnection,
    bus: &Bus,
    client: ClientAddr,
    now: DateTime<Utc>,
) -> Result<SiteId, Error> {
    let subnet = IpNet::V6(LINK_SUBNET);
    if let Some(site) = store::site_for(conn, LINK_SITE, subnet).await? {
        return Ok(site.id);
    }
    let subnet = Cidr::new(subnet).map_err(|_| subnet_refused())?;
    let name = LINK_SITE.to_owned();
    let new = validate_new_site(&NewSiteInput { name, subnet }).map_err(Error::Validation)?;
    let site = store::create_site(conn, SiteId::from(Uuid::now_v7()), &new).await?;
    let entry = trail::site_added(&site);
    let written = hub_trail(bus, client, now).write(conn, entry).await;
    written.map_err(Error::Trail)?;
    Ok(site.id)
}

fn subnet_refused() -> Error {
    Error::Validation(vec![FieldError::new("subnet", "That subnet is not valid.")])
}

fn names_taken() -> Error {
    let message = "That name is already taken. Choose another.";
    Error::Validation(vec![FieldError::new("name", message)])
}

/// A device with the host `localhost` is the hub's, whatever its name (HMD-045).
///
/// # Errors
/// `Validation` when every name is taken; `Trail` when the audit entry cannot be written.
pub async fn ensure_hub_device(
    conn: &mut PgConnection,
    bus: &Bus,
    now: DateTime<Utc>,
) -> Result<(Device, bool), Error> {
    let mut tx = conn.begin().await?;
    store::lock_hub_device(&mut tx).await?;
    if let Some(device) = store::device_at_host(&mut tx, HOST).await? {
        tx.commit().await?;
        return Ok((device, false));
    }
    let site = link_site(&mut tx, bus, HUB_CLIENT, now).await?;
    let taken = store::names_in_use(&mut tx, &names()).await?;
    let form = NewDeviceInput {
        site,
        name: free_name(names(), &taken).ok_or_else(names_taken)?,
        host: HOST.to_owned(),
        kind: KIND.to_owned(),
        os: OperatingSystem::Linux,
        tags: Vec::new(),
        ssh: None,
        remote: RemoteForm::default(),
    };
    let new = validate_new_device(&form).map_err(Error::Validation)?;
    let id = DeviceId::from(Uuid::now_v7());
    let device = store::create_device(&mut tx, id, now, &new).await?;
    let entry = trail::device_added(&device, &new, false);
    let written = hub_trail(bus, HUB_CLIENT, now).write(&mut tx, entry).await;
    written.map_err(Error::Trail)?;
    tx.commit().await?;
    Ok((device, true))
}

/// A computer that runs the link and the daemon with no application is a server in most cases
/// (HM-T33).
const COMPUTER_KIND: &str = "server";

/// A computer on the link with no application (HM-T33, R-240 c); its link address is its host.
pub struct LinkComputer<'a> {
    pub address: IpAddr,
    pub name: &'a str,
    pub os: OperatingSystem,
}

fn computer_names(name: &str) -> Vec<String> {
    let numbered = (2..NAMES).map(|number| format!("{name} ({number})"));
    std::iter::once(name.to_owned()).chain(numbered).collect()
}

/// The device whose host is the link address, else a new one in the site `Link` (HM-T33,
/// HMD-044); the audit entries are written as the hub, from the link address.
///
/// # Errors
/// `Validation` when every name is taken; `Trail` when an audit entry cannot be written.
pub async fn ensure_link_computer(
    conn: &mut PgConnection,
    bus: &Bus,
    computer: &LinkComputer<'_>,
    now: DateTime<Utc>,
) -> Result<(Device, bool), Error> {
    let host = computer.address.to_string();
    let mut tx = conn.begin().await?;
    store::lock_hub_device(&mut tx).await?;
    if let Some(device) = store::device_at_host(&mut tx, &host).await? {
        tx.commit().await?;
        return Ok((device, false));
    }
    let client = ClientAddr(computer.address);
    let site = link_site(&mut tx, bus, client, now).await?;
    let names = computer_names(computer.name);
    let taken = store::names_in_use(&mut tx, &names).await?;
    let form = NewDeviceInput {
        site,
        name: free_name(names, &taken).ok_or_else(names_taken)?,
        host,
        kind: COMPUTER_KIND.to_owned(),
        os: computer.os,
        tags: Vec::new(),
        ssh: None,
        remote: RemoteForm::default(),
    };
    let new = validate_new_device(&form).map_err(Error::Validation)?;
    let device = store::create_device(&mut tx, DeviceId::from(Uuid::now_v7()), now, &new).await?;
    let entry = trail::device_added(&device, &new, false);
    let written = hub_trail(bus, client, now).write(&mut tx, entry).await;
    written.map_err(Error::Trail)?;
    tx.commit().await?;
    Ok((device, true))
}

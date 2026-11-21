//! The devices that the hub adds by itself in the site of the link: the hub as a device of its
//! own list (HMD-045), which a hosted hub adds at its start with no SSH, RDP or VNC, and a
//! computer with no application that claims its daemon enrolment through the link (HM-T33).

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

/// The host of the hub's own device. The device is found by it, so a new name stays.
const HOST: &str = "localhost";
const KIND: &str = "hub";
const NAME: &str = "Hub";
/// How many names [`names`] gives.
const NAMES: usize = 10;
/// The site of the link, as the client makes it for the computers of the link (HMD-044).
const LINK_SITE: &str = "Link";
const LINK_SUBNET: Ipv6Net =
    Ipv6Net::new_assert(Ipv6Addr::new(0xfd41, 0x4e49, 0x4d41, 0, 0, 0, 0, 0), 48);

/// The refusal of each way into the container of the hub (HMD-045).
const NO_WAY_IN: &str = "No session reaches the container of the hub: its device takes no SSH, \
                         RDP or VNC.";

/// Whether `kind` is the kind of the hub's own device, in any letter case.
pub(super) fn is_hub_kind(kind: &str) -> bool {
    kind.trim().eq_ignore_ascii_case(KIND)
}

/// The refusal of the kind `hub` for a device that the operator adds or changes: only the hub
/// adds its own device, and that device keeps its kind.
pub(super) fn kind_refused() -> FieldError {
    FieldError::new(
        "kind",
        "The kind hub is only for the hub's own device, and that device keeps it.",
    )
}

/// [`kind_refused`] for a new device of the kind `hub`; `Ok` for any other kind.
pub(super) fn refuse_new(kind: &str) -> Result<(), FieldError> {
    if is_hub_kind(kind) {
        Err(kind_refused())
    } else {
        Ok(())
    }
}

/// The fields of `change` that a device of kind `current` refuses (HMD-045): no device takes
/// or leaves the kind `hub`, and the hub's device has no SSH, RDP or VNC.
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

/// Refuses `change` of the device `id` when [`refused_change`] does; reads the device only when
/// the change sets its kind or a way in.
///
/// # Errors
/// `Validation` with the refused fields; the errors of [`store::fetch`].
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

/// Refuses a secret of SSH, RDP or VNC for the hub's own device (HMD-045). The field is the
/// device argument of the mutation.
///
/// # Errors
/// `Validation` when `device` is the hub's.
pub(super) fn check_secret(device: &Device) -> Result<(), Error> {
    if is_hub_kind(&device.kind) {
        Err(Error::Validation(vec![FieldError::new(
            "deviceId", NO_WAY_IN,
        )]))
    } else {
        Ok(())
    }
}

/// The names the hub's device can take, the first choice first: `Hub`, `Hub (hosted)`,
/// `Hub (hosted 2)` and so on.
pub(super) fn names() -> Vec<String> {
    let numbered = (2..NAMES).map(|number| format!("{NAME} (hosted {number})"));
    [NAME.to_owned(), format!("{NAME} (hosted)")]
        .into_iter()
        .chain(numbered)
        .collect()
}

/// The first of `names` that is not in `taken`. A device name is unique.
pub(super) fn free_name(names: Vec<String>, taken: &[String]) -> Option<String> {
    names.into_iter().find(|name| !taken.contains(name))
}

/// The hub's own work is written for no operator (LOG-014), from `client`: the hub's own
/// address, or the link address of the computer it acts for.
fn hub_trail(bus: &Bus, client: ClientAddr, now: DateTime<Utc>) -> Trail<'_> {
    Trail::new(bus, (Operator::hub(), client), now)
}

/// The hub's own address.
const HUB_CLIENT: ClientAddr = ClientAddr(IpAddr::V4(Ipv4Addr::LOCALHOST));

/// The site of the link: the one that exists, else a new one with its audit entry.
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

/// Makes sure that the hub is a device of its own list, in the caller's transaction (HMD-045).
/// A device with the host `localhost` is the hub's, whatever its name. When there is none, adds
/// one: the first free name of `Hub`, `Hub (hosted)`…, the kind `hub`, Linux, in the site
/// `Link`, with no SSH, RDP or VNC and no check, and writes its audit entry and event as the
/// hub. Returns the device and whether it was added now.
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

/// The kind of a computer that the hub adds for an installation with no application (HM-T33):
/// a computer that runs the link and the daemon with no application is a server in most cases.
const COMPUTER_KIND: &str = "server";

/// A computer on the link that has no application (HM-T33, R-240 c): its link address, which is
/// its host, its name on the link, and its operating system as the installation says it.
pub struct LinkComputer<'a> {
    pub address: IpAddr,
    pub name: &'a str,
    pub os: OperatingSystem,
}

/// The names that a computer of the link can take, the first choice first: its name on the
/// link, then `<name> (2)` and so on.
fn computer_names(name: &str) -> Vec<String> {
    let numbered = (2..NAMES).map(|number| format!("{name} ({number})"));
    std::iter::once(name.to_owned()).chain(numbered).collect()
}

/// The device of `computer` in the caller's transaction (HM-T33): the device whose host is its
/// link address, else a new one in the site `Link`, as the application adds the computer it
/// runs on (HMD-044), with the kind `server`, no check and no SSH, RDP or VNC. The audit
/// entries are written as the hub, from the link address. Returns the device and whether it
/// was added now.
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

//! Installing a computer with no application (HM-T33, R-240 c). Only the computer that joined
//! with the invite can claim, through its link; the claim gives a daemon enrolment code, never
//! an operator's token. Neither call writes an invite, a claim or a code to a log or an entry
//! (LOG-007).

use std::sync::Arc;

use async_graphql::{Context, Object, Result, SimpleObject};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};

use super::{
    super::{
        claims::{CLAIM_BYTES, ClaimHash, ComputerClaims, OpenClaim},
        model::ListedInvite,
    },
    Change, Control, ListedDevice, MadeInvite, NAME_CHARS, NAME_RULE, Request, change, control_of,
    hosted::CLAIM_LIFE,
    to_graphql,
};
use crate::{
    api_errors::{
        ErrorCode, FieldError, begin, coded, commit, internal, invalid, pool, schema_dependency,
    },
    clock::Clock,
    config::LINK_DEVICES,
    fleet::{
        access::{self, Authenticated, operator_only},
        devices::{self, LinkComputer, OperatingSystem},
        events::{Bus, ClientAddr, Operator, Trail},
        metrics::{self, DaemonEnrolment, DaemonError},
    },
};

const WRONG_CLAIM: &str =
    "The claim is used, expired or wrong, or this computer did not join with its invite.";
const NAME_TWICE: &str = "More than one device of the link has this name. Revoke the one that \
                          is not this computer from the link, then make the install code again.";
const NO_DEVICE_NAME: &str = "The hub cannot add this computer to its devices: each free form of \
                              its link name is taken. Free the name in Devices, then make a new \
                              install code.";
const DEVICE_GONE: &str =
    "The device of this computer was removed meanwhile. Make a new install code.";

/// K1 `ComputerInstall`: shown once to the operator, who passes it to the computer. No `Debug`.
#[derive(SimpleObject)]
pub struct ComputerInstall {
    /// The link invite: works once and expires (LNK-003). Null when a device with this name is
    /// already on the link: the claim then gives that computer a new enrolment code.
    invite: Option<String>,
    /// The one-time claim that the computer that joined with `invite` trades for a daemon
    /// enrolment code (`claimComputerEnrolment`).
    claim: String,
    expires_at: DateTime<Utc>,
}

#[derive(Default)]
pub struct ComputerMutation;

#[Object]
impl ComputerMutation {
    /// An invite and a one-time claim for the computer named `name`, which has no application
    /// (HM-T33). A computer already on the link gets a claim only. A new claim for a name ends
    /// the open claim of that name. Login or browser token only (D-23).
    #[graphql(guard = "Authenticated")]
    async fn create_computer_install(
        &self,
        ctx: &Context<'_>,
        name: String,
    ) -> Result<ComputerInstall> {
        operator_only(ctx, "createComputerInstall")?;
        let length = name.chars().count();
        if length == 0 || length > NAME_CHARS || name.chars().any(char::is_control) {
            return Err(invalid(&[FieldError::new("name", NAME_RULE)]));
        }
        let control = control_of(ctx)?;
        let now = schema_dependency::<Arc<dyn Clock>>(ctx)?.now();
        let devices: Vec<ListedDevice> =
            (control.call(Request::DevicesList).await).map_err(|error| to_graphql(&error, None))?;
        let named = devices.iter().filter(|device| device.name == name).count();
        if named > 1 {
            return Err(coded(NAME_TWICE, ErrorCode::Conflict));
        }
        // An older invite of the name could join a second computer with the same name.
        cancel_open_invites(ctx, &control, &name).await?;
        // The answer holds a secret: no cache may keep it.
        ctx.insert_http_header("cache-control", "no-store");
        let (invite, expires) = if named == 1 {
            claim_made(ctx, &name).await?;
            (None, now + CLAIM_LIFE)
        } else {
            let made = made_invite(ctx, &name).await?;
            (Some(made.invite.expose().to_owned()), made.expires)
        };
        let claim =
            URL_SAFE_NO_PAD.encode(access::random_bytes::<CLAIM_BYTES>().map_err(internal)?);
        let open = OpenClaim {
            device_name: name,
            expires,
        };
        let claims = &schema_dependency::<ComputerClaims>(ctx)?.0;
        claims.open(ClaimHash::of(&claim), open, now);
        Ok(ComputerInstall {
            invite,
            claim,
            expires_at: expires,
        })
    }

    /// A daemon enrolment code for the computer that sends this through the link, when it
    /// joined with the invite of `claim`. The hub adds the computer to its devices when no
    /// device has its link address as its host. The claim works once. Needs no token (HM-T33).
    async fn claim_computer_enrolment(
        &self,
        ctx: &Context<'_>,
        claim: String,
        os: OperatingSystem,
    ) -> Result<DaemonEnrolment> {
        let client = *schema_dependency::<ClientAddr>(ctx)?;
        let address = client.0.to_canonical();
        let from_link = matches!(address, std::net::IpAddr::V6(v6) if LINK_DEVICES.contains(&v6));
        // A sender that is not on the link never reaches the book: its try spends nothing.
        if !from_link {
            return Err(refused());
        }
        let now = schema_dependency::<Arc<dyn Clock>>(ctx)?.now();
        let claims = &schema_dependency::<ComputerClaims>(ctx)?.0;
        let hash = ClaimHash::of(&claim);
        // Spent before the sender is compared, so a wrong sender cannot try it twice; restored
        // only when the right computer then fails on the hub's side.
        let open = claims.use_claim(hash, now).ok_or_else(refused)?;
        let sender = match sender_of(ctx, client).await {
            Ok(sender) => sender,
            Err(error) => {
                claims.restore(hash, open, now);
                return Err(error);
            }
        };
        let sender = sender
            .filter(|device| device.name == open.device_name)
            .ok_or_else(refused)?;
        ctx.insert_http_header("cache-control", "no-store");
        let computer = LinkComputer {
            address,
            name: &sender.name,
            os,
        };
        let enrolment = enrolment_of(ctx, &computer, now).await;
        if enrolment.is_err() {
            claims.restore(hash, open, now);
        }
        enrolment
    }
}

fn refused() -> async_graphql::Error {
    coded(WRONG_CLAIM, ErrorCode::Unauthenticated)
}

/// The device of the link whose address is `client`, if any.
async fn sender_of(ctx: &Context<'_>, client: ClientAddr) -> Result<Option<ListedDevice>> {
    let devices: Vec<ListedDevice> = (control_of(ctx)?.call(Request::DevicesList).await)
        .map_err(|error| to_graphql(&error, None))?;
    Ok(devices
        .into_iter()
        .find(|device| device.address == client.0))
}

/// The device of `computer`, found or added, and a new enrolment code for its daemon, in one
/// transaction, written as the hub from the computer's link address.
async fn enrolment_of(
    ctx: &Context<'_>,
    computer: &LinkComputer<'_>,
    now: DateTime<Utc>,
) -> Result<DaemonEnrolment> {
    let bus = schema_dependency::<Bus>(ctx)?;
    let mut tx = begin(ctx).await?;
    let (device, _) = devices::ensure_link_computer(&mut tx, bus, computer, now)
        .await
        .map_err(device_error)?;
    let trail = Trail::new(bus, (Operator::hub(), ClientAddr(computer.address)), now);
    let enrolment = metrics::enrol_code(&mut tx, &device, trail)
        .await
        .map_err(daemon_error)?;
    commit(tx).await?;
    Ok(enrolment)
}

fn device_error(error: devices::Error) -> async_graphql::Error {
    match error {
        devices::Error::Validation(_) => coded(NO_DEVICE_NAME, ErrorCode::Validation),
        other => internal(other),
    }
}

fn daemon_error(error: DaemonError) -> async_graphql::Error {
    match error {
        DaemonError::NoDevice => coded(DEVICE_GONE, ErrorCode::NotFound),
        other => internal(other),
    }
}

/// Cancels each open invite of the link for `name`, with one audit entry when there was one.
async fn cancel_open_invites(ctx: &Context<'_>, control: &Control, name: &str) -> Result<()> {
    let open: Vec<ListedInvite> =
        (control.call(Request::InvitesList).await).map_err(|error| to_graphql(&error, None))?;
    let mut cancelled = 0_usize;
    for invite in open.iter().filter(|invite| invite.name == name) {
        let cancel = Request::InvitesCancel { id: &invite.id };
        (control.call::<()>(cancel).await).map_err(|error| to_graphql(&error, None))?;
        cancelled += 1;
    }
    if cancelled == 0 {
        return Ok(());
    }
    let trail = Trail::from_context(ctx)?;
    let mut conn = pool(ctx)?.acquire().await.map_err(internal)?;
    let entry = change::invites_cancelled(name, cancelled);
    trail.write(&mut conn, entry).await.map_err(internal)
}

/// A link invite for `name`, with its audit entry; an invite whose entry cannot be written is
/// withdrawn (LOG-005).
async fn made_invite(ctx: &Context<'_>, name: &str) -> Result<MadeInvite> {
    let control = control_of(ctx)?;
    let (asked, entry_name, undo_name, name) = (
        control.clone(),
        name.to_owned(),
        name.to_owned(),
        name.to_owned(),
    );
    change::run(
        ctx,
        Change {
            call: async move {
                let request = Request::InvitesMake { name: &name };
                asked.call::<MadeInvite>(request).await
            },
            field: Some("name"),
            entry_of: move |made: &MadeInvite| change::invite_made(&entry_name, made),
            unwritten: move |made: MadeInvite| async move {
                change::withdrawn(&control, &undo_name, made.expires).await
            },
        },
    )
    .await
}

/// The audit entry of a claim for a computer that is on the link already; never the claim.
async fn claim_made(ctx: &Context<'_>, name: &str) -> Result<()> {
    let trail = Trail::from_context(ctx)?;
    let mut conn = pool(ctx)?.acquire().await.map_err(internal)?;
    let entry = change::install_claim_made(name);
    trail.write(&mut conn, entry).await.map_err(internal)
}

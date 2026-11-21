//! The two calls of the installation of a computer with no application (hosted-mode.md HM-T33,
//! R-240 c): `createComputerInstall`, which a signed-in operator makes, and
//! `claimComputerEnrolment`, which only the computer that joined with that invite can make,
//! through its link. Together they let one line of a shell join the link, add the computer to
//! the devices and enrol its daemon, with no application and no sign-in on the computer. The
//! claim gives a daemon enrolment code only: never a token of an operator.
//!
//! Neither call writes an invite, a claim or a code to a log or an entry (LOG-007).

use std::sync::Arc;

use async_graphql::{Context, Object, Result, SimpleObject};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};

use super::{
    super::claims::{CLAIM_BYTES, ClaimHash, ComputerClaims, OpenClaim},
    Change, ListedDevice, MadeInvite, NAME_CHARS, NAME_RULE, Request, change, control_of,
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
        metrics::{DaemonEnrolment, graphql::computer_enrolment},
    },
};

const WRONG_CLAIM: &str =
    "The claim is used, expired or wrong, or this computer did not join with its invite.";

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
        // The answer holds a secret: no cache may keep it.
        ctx.insert_http_header("cache-control", "no-store");
        let (invite, expires) = if devices.iter().any(|device| device.name == name) {
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
        let refused = || coded(WRONG_CLAIM, ErrorCode::Unauthenticated);
        let address = client.0.to_canonical();
        let from_link = matches!(address, std::net::IpAddr::V6(v6) if LINK_DEVICES.contains(&v6));
        if !from_link {
            return Err(refused());
        }
        let now = schema_dependency::<Arc<dyn Clock>>(ctx)?.now();
        let devices: Vec<ListedDevice> = (control_of(ctx)?.call(Request::DevicesList).await)
            .map_err(|error| to_graphql(&error, None))?;
        let sender = devices
            .into_iter()
            .find(|device| device.address == client.0);
        // The claim is spent before the computer is compared, so a wrong sender cannot try it
        // twice.
        let open = schema_dependency::<ComputerClaims>(ctx)?
            .0
            .use_claim(ClaimHash::of(&claim), now)
            .ok_or_else(refused)?;
        let sender = sender
            .filter(|device| device.name == open.device_name)
            .ok_or_else(refused)?;
        ctx.insert_http_header("cache-control", "no-store");
        let bus = schema_dependency::<Bus>(ctx)?;
        let computer = LinkComputer {
            address,
            name: &sender.name,
            os,
        };
        let mut tx = begin(ctx).await?;
        let (device, _) = devices::ensure_link_computer(&mut tx, bus, &computer, now)
            .await
            .map_err(internal)?;
        let trail = Trail::new(bus, (Operator::hub(), ClientAddr(address)), now);
        let enrolment = computer_enrolment(&mut tx, &device, trail).await?;
        commit(tx).await?;
        Ok(enrolment)
    }
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

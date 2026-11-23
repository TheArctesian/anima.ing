//! `anima vpn invite` and `anima vpn router-key` (VPN-001, VPN-003): the pre-auth key goes only to
//! standard output, once, or to a `0600` file; never to a command line, a log or a note (CMD-014, LOG-007).

use std::fmt::Write as _;

use serde_json::json;
use zeroize::Zeroizing;

use super::documents::{VpnInvite, VpnRouterKey, vpn_invite, vpn_router_key};
use crate::devices::list::site_id;
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::one_time_secret::{Secret, ToFile, Works, show_once};

#[derive(Debug, clap::Args)]
pub struct InviteArgs {
    #[command(flatten)]
    to_file: ToFile,
}

#[derive(Debug, clap::Args)]
pub struct RouterKeyArgs {
    /// The site the subnet router serves, by name or ID.
    #[arg(long)]
    site: String,
    #[command(flatten)]
    to_file: ToFile,
}

impl InviteArgs {
    /// # Errors
    /// [`Failure`] for a hub refusal (the broker's limits) or a file that cannot be written.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        self.to_file.check()?;
        let hub = invocation.hub()?;
        let answer = hub
            .run::<VpnInvite>(vpn_invite::Variables)
            .await?
            .create_vpn_invite;
        let key = Zeroizing::new(answer.key);
        invocation.output.note(&steps(&answer.steps));
        let public = json!({ "expiresAt": answer.expires_at, "steps": answer.steps });
        let secret = Secret {
            label: "pre-auth key",
            field: "key",
            value: &key,
            to_file: &self.to_file,
            works: Works::Once,
        };
        show_once(invocation, &secret, public)
    }
}

impl RouterKeyArgs {
    /// # Errors
    /// [`Failure`] for an unknown site, a hub refusal or a file that cannot be written.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        self.to_file.check()?;
        let hub = invocation.hub()?;
        let site_id = site_id(&hub, &self.site).await?;
        let answer = hub
            .run::<VpnRouterKey>(vpn_router_key::Variables { site_id })
            .await?
            .create_router_key;
        let key = Zeroizing::new(answer.key);
        invocation.output.note(&steps(&answer.steps));
        let public = json!({ "expiresAt": answer.expires_at, "steps": answer.steps });
        let secret = Secret {
            label: "router key",
            field: "key",
            value: &key,
            to_file: &self.to_file,
            works: Works::Once,
        };
        show_once(invocation, &secret, public)
    }
}

fn steps(steps: &[String]) -> String {
    let mut text = String::from("Steps for the new computer, in order:");
    for (number, step) in steps.iter().enumerate() {
        let _ = write!(text, "\n  {}. {step}", number + 1);
    }
    text
}

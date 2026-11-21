//! `anima link install <name>`: the install code of a computer with no application
//! (hosted-mode.md HM-T33, R-240 c). The code holds the link invite and the claim of the daemon
//! enrolment, and is the one secret this command exists to show: on standard output, once, or
//! with `--to-file` in a `0600` file and nowhere else. The install script reads it on standard
//! input (CMD-014, LOG-007).

use serde_json::json;
use zeroize::Zeroizing;

use super::documents::{LinkInstall, link_install};
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::one_time_secret::{Secret, ToFile, Works, show_once};

/// The start of each install code, with the version of its form.
const PREFIX: &str = "anima-install.1.";

#[derive(Debug, clap::Args)]
pub struct InstallArgs {
    /// The computer's name, 1 to 64 visible characters.
    name: String,
    #[command(flatten)]
    to_file: ToFile,
}

/// The install code: the prefix, the claim, and the invite after a dot when the hub gave one.
/// A claim is base64url, so the first dot after the prefix ends it.
fn install_code(claim: &str, invite: Option<&str>) -> String {
    match invite {
        Some(invite) => format!("{PREFIX}{claim}.{invite}"),
        None => format!("{PREFIX}{claim}"),
    }
}

impl InstallArgs {
    /// Makes the invite and the claim and shows them once as one install code; the expiry goes
    /// to standard error (or the `--json` answer).
    ///
    /// # Errors
    /// [`Failure`] for a hub refusal (`VALIDATION` for the name, `FORBIDDEN` for an API key,
    /// `UNAVAILABLE` when the hub has no link) or a file that cannot be written.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        self.to_file.check()?;
        let hub = invocation.hub()?;
        let answer = hub
            .run::<LinkInstall>(link_install::Variables {
                name: self.name.clone(),
            })
            .await?
            .create_computer_install;
        let invite = answer.invite.map(Zeroizing::new);
        let claim = Zeroizing::new(answer.claim);
        let code = Zeroizing::new(install_code(
            &claim,
            invite.as_ref().map(|text| text.as_str()),
        ));
        invocation.output.note(&format!(
            "Install code for the computer {}; it expires {}. On the computer, give it to the install script on standard input.",
            self.name, answer.expires_at
        ));
        let public = json!({
            "name": self.name,
            "expiresAt": answer.expires_at,
            "joins": invite.is_some(),
        });
        let secret = Secret {
            label: "install code",
            field: "code",
            value: &code,
            to_file: &self.to_file,
            works: Works::Once,
        };
        show_once(invocation, &secret, public)
    }
}

#[cfg(test)]
mod tests {
    use super::install_code;

    #[test]
    fn hm_t33_the_code_holds_the_claim_and_the_invite_after_it() {
        assert_eq!(
            install_code("Q2xhaW0", Some("anima-invite.1.Y29kZQ.hub")),
            "anima-install.1.Q2xhaW0.anima-invite.1.Y29kZQ.hub"
        );
        assert_eq!(install_code("Q2xhaW0", None), "anima-install.1.Q2xhaW0");
    }
}

//! Enrol the current private-link computer without exposing either enrolment credential.
use crate::{failure::Failure, hub::HubError, invocation::Invocation};
use graphql_client::GraphQLQuery;
use std::{process::Stdio, time::Duration};
use tokio::{io::AsyncWriteExt as _, process::Command};
use zeroize::Zeroizing;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/daemons/install.graphql"
)]
struct LocalComputerInstall;
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "src/daemons/install.graphql"
)]
struct LocalComputerEnrolment;

#[derive(Debug, clap::Args)]
pub struct InstallArgs {
    /// The same computer name used for browser approval.
    #[arg(long)]
    computer: Option<String>,
}

impl InstallArgs {
    /// # Errors
    /// A missing private link, a refused enrolment, or a failed user service installation.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        if hub.origin().socket_path().is_none() {
            return Err(failed("First sign in with anima auth login --hosted."));
        }
        let os = match std::env::consts::OS {
            "linux" => local_computer_enrolment::OperatingSystem::LINUX,
            "macos" => local_computer_enrolment::OperatingSystem::MACOS,
            _ => return Err(failed("Standalone daemon setup supports Linux and macOS.")),
        };
        let name = self.computer.unwrap_or_else(crate::auth::this_computer);
        let setup = hub
            .run::<LocalComputerInstall>(local_computer_install::Variables { name })
            .await?
            .create_computer_install;
        let claim = Zeroizing::new(setup.claim);
        if setup.invite.is_some() {
            return Err(failed(
                "That name is not this computer's existing private link. Use its original name.",
            ));
        }
        let mut request =
            LocalComputerEnrolment::build_query(local_computer_enrolment::Variables {
                claim: claim.to_string(),
                os,
            });
        let answer = hub
            .send::<_, local_computer_enrolment::ResponseData>(&request)
            .await;
        zeroize::Zeroize::zeroize(&mut request.variables.claim);
        let code = Zeroizing::new(answer?.claim_computer_enrolment.code);
        let origin = hub.origin().as_text();
        run_daemon(&["enrol", "--hub", &origin, "-"], Some(&code)).await?;
        run_daemon(&["install"], None).await?;
        invocation
            .output
            .note("This computer is enrolled. Its daemon is installed as a user service.");
        Ok(())
    }
}

async fn run_daemon(args: &[&str], input: Option<&str>) -> Result<(), Failure> {
    let program = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("anima-daemon")))
        .filter(|path| path.is_file())
        .ok_or_else(|| {
            failed("Install the Anima release bundle before enrolling this computer.")
        })?;
    let mut child = Command::new(program)
        .args(args)
        .kill_on_drop(true)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| failed("The daemon could not start."))?;
    if let Some(input) = input {
        let mut stdin = child.stdin.take().ok_or(HubError::Setup)?;
        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|_| failed("The daemon could not read the enrolment."))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|_| failed("The daemon could not read the enrolment."))?;
    }
    let result = tokio::time::timeout(Duration::from_secs(45), child.wait())
        .await
        .map_err(|_| failed("The daemon setup timed out."))?
        .map_err(|_| failed("The daemon setup failed."))?;
    if !result.success() {
        return Err(failed(
            "The daemon refused setup. Check the user service manager and retry.",
        ));
    }
    Ok(())
}
fn failed(message: &str) -> Failure {
    Failure::Usage {
        message: message.to_owned(),
        hint: None,
    }
}

use super::{device, login::SignedIn, remote_link::RemoteLink};
use crate::hub::scalars::DateTime;
use crate::{
    failure::Failure,
    hub::{Hub, HubError, Origin, Token, account},
    invocation::Invocation,
};
use graphql_client::GraphQLQuery;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

#[derive(GraphQLQuery)]
#[graphql(schema_path = "schema.graphql", query_path = "src/auth/hosted.graphql")]
struct ClaimCommandLineToken;

pub(super) async fn sign_in(
    invocation: &Invocation,
    web: bool,
    computer: &str,
) -> Result<(Origin, SignedIn), Failure> {
    let link = RemoteLink::prepare(invocation.config_dir()?).await?;
    let started = account::start(computer).await?;
    let code = Zeroizing::new(started.device_code);
    if started.verification_uri != "https://anima.ing/device" {
        return Err(HubError::Malformed { status: 200 }.into());
    }
    device::show_code(
        invocation,
        web,
        &started.user_code,
        &started.verification_uri,
    )?;
    let access = wait(code, started.interval).await?;
    let origin = link.connect(&access).await?;
    let hub = Hub::new(origin.clone(), None)?;
    let mut request = ClaimCommandLineToken::build_query(claim_command_line_token::Variables {
        claim: access.claim.clone(),
    });
    let result = hub
        .send::<_, claim_command_line_token::ResponseData>(&request)
        .await;
    zeroize::Zeroize::zeroize(&mut request.variables.claim);
    let answer = result?.claim_command_line_token;
    let token =
        Token::new(Zeroizing::new(answer.token)).ok_or(HubError::Malformed { status: 200 })?;
    Ok((
        origin,
        SignedIn {
            token,
            username: answer.operator.username,
        },
    ))
}

async fn wait(code: Zeroizing<String>, initial: u64) -> Result<account::Access, Failure> {
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut interval = initial;
    loop {
        if !(5..=600).contains(&interval)
            || Instant::now() + Duration::from_secs(interval) >= deadline
        {
            return Err(device::ended(
                "The sign-in code expired. Run anima auth login --hosted again.",
            ));
        }
        tokio::time::sleep(Duration::from_secs(interval)).await;
        let answer = account::poll(&code).await?;
        interval = interval.max(answer.interval);
        match answer.status.as_str() {
            "WAITING" | "SLOW_DOWN" => {}
            "DENIED" => return Err(device::ended("The sign-in was denied.")),
            "EXPIRED" => return Err(device::ended("The sign-in code expired.")),
            "APPROVED" => {
                return answer
                    .access
                    .ok_or_else(|| HubError::Malformed { status: 200 }.into());
            }
            _ => return Err(HubError::Malformed { status: 200 }.into()),
        }
    }
}

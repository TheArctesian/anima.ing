//! A reload reuses the linked credential for this exact app session rather than revoking active CLI work.
use super::documents::{Me, me};
use crate::{
    failure::Failure,
    hub::{Hub, HubError, Origin, Token},
    invocation::Invocation,
    private_file,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use graphql_client::GraphQLQuery;
use sha2::{Digest as _, Sha256};
use zeroize::Zeroizing;

#[derive(GraphQLQuery)]
#[graphql(schema_path = "schema.graphql", query_path = "src/auth/adopt.graphql")]
struct AdoptAppSession;

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Receipt {
    parent: String,
    child: String,
}

fn hash(value: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(value.as_bytes()))
}
fn name(origin: &Origin) -> String {
    format!("app-session-{}.json", hash(&origin.as_text()))
}

pub(super) async fn linked(
    invocation: &Invocation,
    parent: Hub,
) -> Result<(Hub, Receipt), Failure> {
    let parent_hash = hash(parent.token().ok_or(HubError::Setup)?.expose());
    if let Some((hub, receipt)) = cached(invocation, parent.origin(), &parent_hash)?
        && hub.run::<Me>(me::Variables).await.is_ok()
    {
        return Ok((hub, receipt));
    }
    let computer = crate::auth::this_computer();
    let answer = parent
        .run::<AdoptAppSession>(adopt_app_session::Variables { computer })
        .await?;
    let token = Token::new(Zeroizing::new(answer.issue_command_line_token.token))
        .ok_or(HubError::Malformed { status: 200 })?;
    let receipt = Receipt {
        parent: parent_hash,
        child: hash(token.expose()),
    };
    Ok((Hub::new(parent.origin().clone(), Some(token))?, receipt))
}

fn cached(
    invocation: &Invocation,
    origin: &Origin,
    parent_hash: &str,
) -> Result<Option<(Hub, Receipt)>, Failure> {
    let file = invocation.config_dir()?.join(name(origin));
    let receipt = private_file::read(&file)
        .map_err(|_| super::device::ended("The app sign-in receipt could not be read."))?;
    let Some(receipt) = receipt.and_then(|bytes| serde_json::from_slice::<Receipt>(&bytes).ok())
    else {
        return Ok(None);
    };
    let Some(token) = invocation.token_store().read(origin)? else {
        return Ok(None);
    };
    if receipt.parent != parent_hash || receipt.child != hash(token.expose()) {
        return Ok(None);
    }
    Ok(Some((Hub::new(origin.clone(), Some(token))?, receipt)))
}

pub(super) fn save(
    invocation: &Invocation,
    origin: &Origin,
    receipt: &Receipt,
) -> Result<(), Failure> {
    let bytes = serde_json::to_vec(receipt).map_err(|_| HubError::Setup)?;
    private_file::write(invocation.config_dir()?, &name(origin), &bytes)
        .map_err(|_| super::device::ended("The app sign-in receipt could not be saved."))
}

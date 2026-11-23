//! A credential comes from a prompt, standard input or a private file, never an argument (CMD-025).

use std::path::Path;

use graphql_client::GraphQLQuery;
use zeroize::Zeroizing;

use crate::failure::Failure;
use crate::hub::{Hub, HubError};
use crate::input;
use crate::prompt::Prompter;

const STDIN: &str = "-";

/// The longest password or key; a private key file is a few kilobytes.
const LIMIT: usize = 64 * 1024;

/// # Errors
/// [`Failure::Usage`] when it cannot ask; [`Failure::Input`] for an unusable file.
pub fn password(
    prompter: Prompter,
    what: &str,
    from_file: Option<&Path>,
) -> Result<Zeroizing<String>, Failure> {
    let mut text = match from_file {
        Some(path) => read(prompter, path, what)?,
        None => prompter.ask_secret(what, "--from-file")?,
    };
    let kept = text.trim_end_matches(['\n', '\r']).len();
    text.truncate(kept);
    nonempty(text, what)
}

/// A secret that cannot be typed on one line (a private key) from `from_file`.
///
/// # Errors
/// As [`password`]; [`Failure::Usage`] when `from_file` is missing.
pub fn from_file_only(
    prompter: Prompter,
    what: &str,
    from_file: Option<&Path>,
) -> Result<Zeroizing<String>, Failure> {
    let path = from_file.ok_or_else(|| Failure::missing(what, "--from-file"))?;
    nonempty(read(prompter, path, what)?, what)
}

/// Wipes the secrets in `variables` with `wipe` whether or not the hub accepted the request.
///
/// # Errors
/// [`HubError`] for a refusal or a failed request.
pub async fn send_wiping<Q: GraphQLQuery>(
    hub: &Hub,
    variables: Q::Variables,
    wipe: impl FnOnce(&mut Q::Variables),
) -> Result<Q::ResponseData, HubError> {
    let mut body = Q::build_query(variables);
    let answer = hub.send(&body).await;
    wipe(&mut body.variables);
    answer
}

fn nonempty(text: Zeroizing<String>, what: &str) -> Result<Zeroizing<String>, Failure> {
    if text.is_empty() {
        return Err(unreadable(what, "it is empty"));
    }
    Ok(text)
}

/// The reason in a failure never holds the text.
fn read(prompter: Prompter, path: &Path, what: &str) -> Result<Zeroizing<String>, Failure> {
    let bytes = if path.as_os_str() == STDIN {
        input::read(prompter, path, what, LIMIT)?
    } else {
        crate::private_file::read(path)
            .map_err(|error| unreadable(what, &format!("{}: {error}", path.display())))?
            .ok_or_else(|| unreadable(what, &format!("{} does not exist", path.display())))?
    };
    if bytes.len() > LIMIT {
        return Err(unreadable(what, "it is longer than 64 KiB"));
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| unreadable(what, "it is not text"))?;
    Ok(Zeroizing::new(text.to_owned()))
}

fn unreadable(what: &str, reason: &str) -> Failure {
    Failure::Input {
        what: what.to_owned(),
        reason: reason.to_owned(),
    }
}

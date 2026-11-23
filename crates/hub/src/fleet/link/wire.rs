//! The control socket's wire form (`link/control/control.go`): one JSON line out, one in.
//! A parse error never carries the line's text: serde quotes values, and one can be an invite (LOG-007).

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};

use super::{Error, Refusal};

/// The longest request or reply line, without its newline (`maxLine` of the link).
pub(super) const MAX_LINE: usize = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Request<'a> {
    DevicesList,
    DevicesRevoke { key: &'a str },
    InvitesMake { name: &'a str },
    InvitesList,
    InvitesCancel { id: &'a str },
}

impl Request<'_> {
    /// A verb with no arguments sends none: the link refuses an argument field it does not know.
    pub fn line(self) -> Vec<u8> {
        let request: Value = match self {
            Self::DevicesList => json!({ "verb": "devices.list" }),
            Self::DevicesRevoke { key } => {
                json!({ "verb": "devices.revoke", "args": { "key": key } })
            }
            Self::InvitesMake { name } => {
                json!({ "verb": "invites.make", "args": { "name": name } })
            }
            Self::InvitesList => json!({ "verb": "invites.list" }),
            Self::InvitesCancel { id } => {
                json!({ "verb": "invites.cancel", "args": { "id": id } })
            }
        };
        let mut line = request.to_string().into_bytes();
        line.push(b'\n');
        line
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply<T> {
    ok: bool,
    #[serde(default = "none")]
    result: Option<T>,
    #[serde(default)]
    error: Option<Refused>,
}

/// `Option::None` without a `T: Default` bound.
const fn none<T>() -> Option<T> {
    None
}

/// The message is not read: the hub says the refusal in its own words.
#[derive(Deserialize)]
struct Refused {
    code: String,
}

pub(super) fn parse_reply<T: DeserializeOwned>(line: &[u8]) -> Result<T, Error> {
    let reply: Reply<T> = serde_json::from_slice(line).map_err(|_| Error::Unreadable)?;
    match (reply.ok, reply.result, reply.error) {
        (true, Some(value), None) => Ok(value),
        // A verb with no answer (`invites.cancel`) replies `"result":null`.
        (true, None, None) => serde_json::from_value(Value::Null).map_err(|_| Error::Unreadable),
        (false, None, Some(refused)) => Err(Error::Refused(Refusal::of_code(&refused.code))),
        _ => Err(Error::Unreadable),
    }
}

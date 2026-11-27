use super::{Error, Result, available, enroll, graph, lock, private_link, text};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::AsyncReadExt as _;
use zeroize::Zeroizing;

const CODE_LIMIT: usize = 4096;
const INVALID: Error = Error::Refused("Enter an install code from your Anima hub.");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    code: String,
}

struct Code<'a> {
    claim: &'a str,
    invite: Option<&'a str>,
}

impl<'a> Code<'a> {
    fn parse(text: &'a str) -> Result<Self> {
        let text = text.trim();
        if text.len() > CODE_LIMIT {
            return Err(INVALID);
        }
        let rest = text.strip_prefix("anima-install.1.").ok_or(INVALID)?;
        let (claim, invite) = rest
            .split_once('.')
            .map_or((rest, None), |(claim, invite)| (claim, Some(invite)));
        if claim.len() != 43 || !claim.bytes().all(base64url) {
            return Err(INVALID);
        }
        if invite.is_some_and(|invite| {
            !invite.strip_prefix("anima-invite.1.").is_some_and(|body| {
                !body.is_empty() && body.bytes().all(|byte| byte == b'.' || base64url(byte))
            })
        }) {
            return Err(INVALID);
        }
        Ok(Self { claim, invite })
    }
}

fn base64url(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
}

pub(in crate::linux) async fn from_console(args: Value) -> Result<Value> {
    let input: Input = serde_json::from_value(args)?;
    enroll_install(&Zeroizing::new(input.code)).await
}

pub(in crate::linux) async fn from_stdin() -> Result<()> {
    let mut code = Zeroizing::new(String::new());
    tokio::io::stdin()
        .take((CODE_LIMIT + 3) as u64)
        .read_to_string(&mut code)
        .await?;
    if code.len() > CODE_LIMIT + 2 {
        return Err(INVALID);
    }
    enroll_install(&code).await?;
    println!("Enrollment saved. Check the connection with anima-status.");
    Ok(())
}

async fn enroll_install(input: &str) -> Result<Value> {
    let code = Code::parse(input)?;
    let _lease = lock("/run/anima-console-enrollment.lock")?;
    available()?;
    match (private_link::paired(), code.invite) {
        (false, Some(invite)) => private_link::join(invite).await.map_err(|_| Error::Refused(
            "The private link could not join. Check the connection and make a fresh install code.",
        ))?,
        (true, None) => (),
        (true, Some(_)) => {
            return Err(Error::Refused(
                "This computer is already on a private link. Make a new install code for its existing link name.",
            ));
        }
        (false, None) => {
            return Err(Error::Refused(
                "This code is for a computer already on the link. Make an install code with a new device name.",
            ));
        }
    }
    private_link::start().await?;
    let answer = graph(
        "mutation($claim:String!){claimComputerEnrolment(claim:$claim,os:LINUX){code}}",
        json!({"claim":code.claim}),
        None,
    )
    .await
    .map_err(|_| Error::Refused(
        "The hub could not complete this claim. Make a new install code for this computer's link name.",
    ))?;
    let enrollment = Zeroizing::new(text(&answer["claimComputerEnrolment"], "code")?.to_owned());
    enroll(&enrollment, private_link::ORIGIN).await
}

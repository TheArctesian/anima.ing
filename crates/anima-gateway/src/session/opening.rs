//! A session from the hub's first instruction to a signed-in device (GW-015).
//! Each failure tells the hub the status that gives the session its end reason (K4 Section 5).

use guac_protocol::Instruction;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::time::timeout;
use zeroize::{Zeroize, Zeroizing};

use super::hub::Hub;
use super::{Ended, Settings, status};
use crate::frames::Formats;
use crate::handshake::{
    self, Credential, Handshake, Opening, Prompt, PromptStep, Refused, Secret, Step,
};
use crate::vnc::{self, Account, SignInError};

/// The most instructions a hub sends before `connect`: seven, and room for `nop`.
const MOST_BEFORE_CONNECT: usize = 32;
/// Random bytes for one sign-in: the longest secret number and the filler of Apple's method.
const RANDOM_BYTES: usize = 512 + 128;

/// Holds no account, so the account is dropped and zeroed when the sign-in ends.
#[derive(Clone, Copy)]
pub(super) struct Allowed {
    /// The device's clipboard may go to the client (RA-033).
    pub(super) copy: bool,
    /// The client's clipboard may go to the device (RA-033).
    pub(super) paste: bool,
    pub(super) formats: Formats,
    /// The client's view, in pixels.
    pub(super) view: (usize, usize),
}

impl From<&Opening> for Allowed {
    fn from(opening: &Opening) -> Self {
        Self {
            copy: opening.copy,
            paste: opening.paste,
            formats: opening.formats,
            view: opening.view,
        }
    }
}

pub(super) trait DeviceStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> DeviceStream for T {}
pub(super) type Connection = Box<dyn DeviceStream>;

pub(super) struct Opened {
    pub(super) allowed: Allowed,
    pub(super) device: Connection,
    pub(super) size: (usize, usize),
}

pub(super) enum First {
    Vnc(Opening),
    /// `select` of a protocol this gateway does not serve or passes on, for another gateway.
    Other(Instruction),
}

pub(super) async fn handshake<S>(
    hub: &mut Hub<S>,
    connection_id: &str,
    passed: &[String],
) -> Result<First, Ended>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let mut handshake = Handshake::default();
    for _ in 0..MOST_BEFORE_CONNECT {
        let instruction = hub.next().await.ok_or(Ended::ByHub)?;
        let select = instruction.opcode == "select";
        if select
            && instruction
                .args
                .first()
                .is_some_and(|name| passed.contains(name))
        {
            return Ok(First::Other(instruction));
        }
        let kept = select.then(|| instruction.clone());
        match handshake.on_instruction(instruction) {
            Ok(Step::Send(args)) => hub.send(&[args]).await.map_err(|_| Ended::ByHub)?,
            Ok(Step::Read) => {}
            Ok(Step::Opened(opening)) => {
                let ready = handshake::ready(connection_id);
                hub.send(&[ready]).await.map_err(|_| Ended::ByHub)?;
                return Ok(First::Vnc(*opening));
            }
            Err(Refused::NotServed) => return kept.map(First::Other).ok_or(Ended::Refused),
            Err(refused) => return Err(refuse(hub, refused.status(), Ended::Refused).await),
        }
    }
    Err(refuse(hub, Refused::Unexpected.status(), Ended::Refused).await)
}

pub(super) async fn refuse<S>(hub: &mut Hub<S>, status: u16, ended: Ended) -> Ended
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    // The end stands whether or not the hub still hears of it.
    let _ = hub.send(&[handshake::error(status)]).await;
    ended
}

pub(super) async fn sign_in<S>(
    hub: &mut Hub<S>,
    mut opening: Opening,
    settings: &Settings,
) -> Result<Opened, Ended>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let mut random = Zeroizing::new([0u8; RANDOM_BYTES]);
    if getrandom::fill(&mut *random).is_err() {
        return Err(refuse(hub, status::SERVER_ERROR, Ended::NoRandomNumbers).await);
    }
    let connecting = timeout(settings.device_wait, TcpStream::connect(opening.device)).await;
    let Ok(Ok(mut device)) = connecting else {
        return Err(refuse(hub, status::UPSTREAM_NOT_FOUND, Ended::DeviceUnreachable).await);
    };
    // Keys and pointer moves are small: do not hold them back (Nagle).
    let _ = device.set_nodelay(true);
    let Ok(Ok(offered)) = timeout(settings.device_wait, vnc::offered(&mut device)).await else {
        return Err(refuse(hub, status::UPSTREAM_ERROR, Ended::NotVnc).await);
    };
    if opening.vnc_certificate.is_some() {
        if !offered.supports_vencrypt() {
            return Err(refuse(hub, status::UPSTREAM_ERROR, Ended::NoSignInMethod).await);
        }
        return super::vencrypt::open(hub, opening, device, settings).await;
    }
    let Ok(method) = offered.method(opening.username.is_some(), opening.password.is_some()) else {
        return Err(refuse(hub, status::UPSTREAM_ERROR, Ended::NoSignInMethod).await);
    };
    ask_for_missing(hub, &mut opening, method.needs(), settings).await?;
    let text = |secret: &Option<Secret>| secret.as_ref().map_or("", Secret::expose).to_owned();
    let (username, password) = (
        Zeroizing::new(text(&opening.username)),
        Zeroizing::new(text(&opening.password)),
    );
    let account = Account {
        username: &username,
        password: &password,
    };
    let mut left = &random[..];
    let mut short = false;
    let mut hand_out = |buffer: &mut [u8]| {
        let (taken, rest) = left.split_at(buffer.len().min(left.len()));
        short |= taken.len() < buffer.len();
        buffer[..taken.len()].copy_from_slice(taken);
        left = rest;
    };
    let signing = vnc::sign_in(&mut device, method, account, &mut hand_out);
    let signed_in = timeout(settings.device_wait, signing).await;
    let allowed = Allowed::from(&opening);
    match signed_in {
        // A sign-in that asked for more random bytes than were made used zeros for the rest.
        Ok(Ok(_)) if short => Err(refuse(hub, status::SERVER_ERROR, Ended::NoRandomNumbers).await),
        Ok(Ok(size)) => Ok(Opened {
            allowed,
            device: Box::new(device),
            size,
        }),
        Ok(Err(SignInError::Refused)) => {
            Err(refuse(hub, status::CLIENT_UNAUTHORIZED, Ended::SignInRefused).await)
        }
        _ => Err(refuse(hub, status::UPSTREAM_ERROR, Ended::SignInFailed).await),
    }
}

pub(super) async fn ask_for_missing<S>(
    hub: &mut Hub<S>,
    opening: &mut Opening,
    needs: &[Credential],
    settings: &Settings,
) -> Result<(), Ended>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let has = |credential: &Credential| match credential {
        Credential::Username => opening.username.is_some(),
        Credential::Password => opening.password.is_some(),
    };
    let missing: Vec<Credential> = needs
        .iter()
        .copied()
        .filter(|credential| !has(credential))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let (mut prompt, required) = Prompt::ask(&missing);
    hub.send(&[required]).await.map_err(|_| Ended::ByHub)?;
    let answers = async {
        while !prompt.is_answered() {
            let mut instruction = hub.next().await.ok_or(Ended::ByHub)?;
            let step = prompt.on_instruction(&instruction);
            let disconnect = instruction.opcode == "disconnect";
            // An answer's `blob` holds a part of the credential.
            instruction.args.iter_mut().for_each(Zeroize::zeroize);
            let reply = match step {
                PromptStep::NotAnAnswer if disconnect => return Err(Ended::ByHub),
                PromptStep::NotAnAnswer => continue,
                // A refused answer: the hub lets the client answer only a new `required`.
                PromptStep::Acknowledge(ack)
                    if ack.args.get(2).is_some_and(|status| status != "0") =>
                {
                    vec![ack, prompt.required()]
                }
                PromptStep::Acknowledge(ack) => vec![ack],
                PromptStep::Answered {
                    credential,
                    value,
                    ack,
                } => {
                    match credential {
                        Credential::Username => opening.username = Some(value),
                        Credential::Password => opening.password = Some(value),
                    }
                    vec![ack]
                }
            };
            hub.send(&reply).await.map_err(|_| Ended::ByHub)?;
        }
        Ok(())
    };
    match timeout(settings.answer_wait, answers).await {
        Ok(answered) => answered,
        Err(_) => Err(refuse(hub, status::CLIENT_TIMEOUT, Ended::NoAnswer).await),
    }
}

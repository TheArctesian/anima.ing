//! Pinned VNC transport is established before prompting for or sending credentials.
use super::{
    Ended, Settings,
    hub::Hub,
    opening::{Allowed, Opened, ask_for_missing, refuse},
    status,
};
use crate::{
    handshake::{Credential, Opening, Secret},
    vnc::{self, Account},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    time::timeout,
};

pub(super) async fn open<S>(
    hub: &mut Hub<S>,
    mut opening: Opening,
    device: TcpStream,
    settings: &Settings,
) -> Result<Opened, Ended>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let pin = opening.vnc_certificate.ok_or(Ended::Refused)?;
    let Ok(Ok(mut device)) = timeout(settings.device_wait, vnc::secure(device, pin)).await else {
        return Err(refuse(hub, status::UPSTREAM_ERROR, Ended::SignInFailed).await);
    };
    ask_for_missing(
        hub,
        &mut opening,
        &[Credential::Username, Credential::Password],
        settings,
    )
    .await?;
    let account = Account {
        username: opening.username.as_ref().map_or("", Secret::expose),
        password: opening.password.as_ref().map_or("", Secret::expose),
    };
    let signed = timeout(
        settings.device_wait,
        vnc::plain_sign_in(&mut device, account),
    )
    .await;
    match signed {
        Ok(Ok(size)) => Ok(Opened {
            allowed: Allowed::from(&opening),
            device: Box::new(device),
            size,
        }),
        Ok(Err(vnc::SignInError::Refused)) => {
            Err(refuse(hub, status::CLIENT_UNAUTHORIZED, Ended::SignInRefused).await)
        }
        _ => Err(refuse(hub, status::UPSTREAM_ERROR, Ended::SignInFailed).await),
    }
}

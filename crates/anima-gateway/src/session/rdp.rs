//! An RDP session from `connect` to its end; each opening failure reaches the hub as the status
//! of its end reason (K4 Section 5).

mod running;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::hub::Hub;
use super::opening::{Allowed, ask_for_missing, refuse};
use super::{Ended, Settings, status};
use crate::handshake::{Credential, Opening, Rdp, Secret};
use crate::rdp::{ClipboardNotes, OpenError, Wanted, open, screen_for};

struct Terms {
    allowed: Allowed,
    /// The device is asked for a screen of the size of the client's view.
    resizes: bool,
}

pub(super) async fn serve<S>(
    hub: &mut Hub<S>,
    mut opening: Opening,
    rdp: &Rdp,
    settings: &Settings,
) -> Ended
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    // NLA takes the whole account at once; asking before the device is reached keeps it from waiting open (GW-015).
    if rdp.security.may_be_nla() {
        let account = [Credential::Username, Credential::Password];
        if let Err(ended) = ask_for_missing(hub, &mut opening, &account, settings).await {
            return ended;
        }
    }
    let connecting = timeout(settings.device_wait, TcpStream::connect(opening.device)).await;
    let Ok(Ok(device)) = connecting else {
        return refuse(hub, status::UPSTREAM_NOT_FOUND, Ended::DeviceUnreachable).await;
    };
    // Keys and pointer moves are small: do not hold them back (Nagle).
    let _ = device.set_nodelay(true);
    let wanted = Wanted {
        username: text(opening.username.as_ref()),
        password: text(opening.password.as_ref()),
        domain: rdp.domain.as_deref(),
        security: rdp.security,
        certificate: rdp.certificate,
        size: screen_for(opening.view),
    };
    // The device's offer and its sign-in: each has the wait of a step.
    let opened = timeout(
        2 * settings.device_wait,
        open(device, wanted, ClipboardNotes::default()),
    )
    .await;
    let terms = Terms {
        allowed: Allowed {
            copy: opening.copy,
            paste: opening.paste,
            formats: opening.formats,
            view: opening.view,
        },
        resizes: rdp.resize,
    };
    // The gateway's copies of the account are zeroed here.
    drop(opening);
    let (status, ended) = match opened {
        Ok(Ok(opened)) => return running::run(hub, opened, terms, settings).await,
        Ok(Err(OpenError::Refused)) => (status::CLIENT_UNAUTHORIZED, Ended::SignInRefused),
        Ok(Err(OpenError::NotRdp)) => (status::UPSTREAM_ERROR, Ended::NotRdp),
        Ok(Err(OpenError::Security)) => (status::UPSTREAM_ERROR, Ended::SecurityNotOffered),
        // The hub reads the certificate first (K4 Section 6); a change between the reads is a gateway error.
        Ok(Err(OpenError::Certificate)) => (status::UPSTREAM_ERROR, Ended::OtherCertificate),
        Ok(Err(OpenError::Failed)) | Err(_) => (status::UPSTREAM_ERROR, Ended::SignInFailed),
    };
    refuse(hub, status, ended).await
}

fn text(secret: Option<&Secret>) -> &str {
    secret.map_or("", Secret::expose)
}

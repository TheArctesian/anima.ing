//! `VeNCrypt` 0.2 `X509Plain`: the accepted leaf is checked before Plain credentials.
use super::{Account, SignInError, sign_in};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

const X509_PLAIN: u32 = 262;

pub(crate) async fn secure(
    mut device: TcpStream,
    pin: [u8; 32],
) -> Result<TlsStream<TcpStream>, SignInError> {
    device.write_u8(19).await?;
    let version = device.read_u16().await?;
    if version < 2 {
        return Err(SignInError::NoMethod);
    }
    device.write_all(&[0, 2]).await?;
    if device.read_u8().await? != 0 {
        return Err(SignInError::NoMethod);
    }
    let count = device.read_u8().await?;
    let mut supported = false;
    for _ in 0..count {
        supported |= device.read_u32().await? == X509_PLAIN;
    }
    if !supported {
        return Err(SignInError::NoMethod);
    }
    device.write_u32(X509_PLAIN).await?;
    if device.read_u8().await? == 0 {
        return Err(SignInError::Refused);
    }
    crate::tls::connect(device, crate::handshake::Certificate::Accepted(pin))
        .await
        .map_err(|_| SignInError::Refused)
}

pub(crate) async fn plain_sign_in<S>(
    device: &mut S,
    account: Account<'_>,
) -> Result<(usize, usize), SignInError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let username = u32::try_from(account.username.len()).map_err(|_| SignInError::TooLong)?;
    let password = u32::try_from(account.password.len()).map_err(|_| SignInError::TooLong)?;
    if username > 4096 || password > 4096 {
        return Err(SignInError::TooLong);
    }
    device.write_u32(username).await?;
    device.write_u32(password).await?;
    device.write_all(account.username.as_bytes()).await?;
    device.write_all(account.password.as_bytes()).await?;
    sign_in::finish(device, false).await
}

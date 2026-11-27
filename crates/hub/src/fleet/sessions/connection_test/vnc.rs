//! VNC discovery negotiates `X509Plain` only to observe its TLS leaf; no account data is sent.
//! Nothing here logs, and no error holds the banner.

use std::{io, time::Duration};

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use super::reach::{ReadError, reach, within};
use crate::fleet::{ssh::Target, targets::Reached, trust::PresentedCertificate};

pub(in crate::fleet::sessions) const BANNER: usize = 12;

pub(in crate::fleet::sessions) fn is_banner(bytes: &[u8; BANNER]) -> bool {
    let digits = |range: std::ops::Range<usize>| bytes[range].iter().all(u8::is_ascii_digit);
    bytes.starts_with(b"RFB ")
        && digits(4..7)
        && bytes[7] == b'.'
        && digits(8..11)
        && bytes[11] == b'\n'
}

pub(in crate::fleet::sessions) async fn read_certificate(
    target: Target<'_>,
    limit: Duration,
) -> Result<Option<PresentedCertificate>, ReadError> {
    within(limit, async {
        let mut stream = reach(target).await?;
        if !offers_tls(&mut stream)
            .await
            .map_err(|_| ReadError::Unexpected)?
        {
            return Ok(None);
        }
        vencrypt(&mut stream).await.map_err(|_| ReadError::Tls)?;
        super::tls::leaf(stream).await.map(Some)
    })
    .await
}

async fn offers_tls(stream: &mut Reached) -> io::Result<bool> {
    let mut banner = [0; BANNER];
    stream.read_exact(&mut banner).await?;
    if !is_banner(&banner) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    if &banner[4..7] != b"003" || &banner[8..11] < b"007".as_slice() {
        return Ok(false);
    }
    let version = if &banner[8..11] == b"007" {
        b"RFB 003.007\n"
    } else {
        b"RFB 003.008\n"
    };
    stream.write_all(version).await?;
    let count = stream.read_u8().await?;
    if count == 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let mut offered = vec![0; usize::from(count)];
    stream.read_exact(&mut offered).await?;
    Ok(offered.contains(&19))
}

async fn vencrypt(stream: &mut Reached) -> io::Result<()> {
    stream.write_u8(19).await?;
    if stream.read_u16().await? < 2 {
        return Err(io::ErrorKind::Unsupported.into());
    }
    stream.write_all(&[0, 2]).await?;
    if stream.read_u8().await? != 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let count = stream.read_u8().await?;
    let mut supported = false;
    for _ in 0..count {
        supported |= stream.read_u32().await? == 262;
    }
    if !supported {
        return Err(io::ErrorKind::Unsupported.into());
    }
    stream.write_u32(262).await?;
    if stream.read_u8().await? == 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}

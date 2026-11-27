use super::LinkError;
use rustls::pki_types::CertificateDer;
use sha2::{Digest as _, Sha256};

use crate::threads::holder_channel;
use std::{pin::Pin, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::UnixStream,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::threads) struct Origin(Option<[u8; 32]>);

impl Origin {
    pub(in crate::threads) fn of(root: Option<&CertificateDer<'_>>) -> Self {
        Self(root.map(|root| Sha256::digest(root.as_ref()).into()))
    }
}

type Reading = Pin<Box<dyn AsyncRead + Send>>;
type Writing = Pin<Box<dyn AsyncWrite + Send + Sync>>;

pub(super) async fn open(
    stream: UnixStream,
    client: Option<rustls::ClientConfig>,
) -> Result<(Reading, Writing), LinkError> {
    if let Some(config) = client {
        let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
        let name = rustls::pki_types::ServerName::try_from(holder_channel::NAME)
            .expect("fixed holder name");
        let channel = tokio::time::timeout(Duration::from_secs(3), connector.connect(name, stream))
            .await
            .map_err(|_| LinkError::NoHolder)??;
        let (read, write) = tokio::io::split(channel);
        Ok((Box::pin(read), Box::pin(write)))
    } else {
        let (read, write) = stream.into_split();
        Ok((Box::pin(read), Box::pin(write)))
    }
}

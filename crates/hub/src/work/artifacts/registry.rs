//! Send a verified image to one configured private OCI registry by digest.
mod destination;
#[cfg(test)]
mod tests;
mod transport;

use std::{fs::File, sync::Arc, time::Duration};

use oci_artifact::Publication;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::Error;
use crate::config::ArtifactRegistry;
use destination::Destination;
use transport::Transport;

pub(super) struct Publisher {
    transport: Transport,
    turn: Arc<Semaphore>,
}

impl Publisher {
    pub(super) fn open(settings: &ArtifactRegistry) -> Result<Self, Error> {
        let destination = Destination::parse(&settings.origin, &settings.repository)?;
        let authorization = settings
            .authorization
            .as_deref()
            .map(destination::authorization)
            .transpose()?;
        Self::new(destination, authorization)
    }

    fn new(destination: Destination, authorization: Option<HeaderValue>) -> Result<Self, Error> {
        let mut headers = HeaderMap::new();
        if let Some(authorization) = authorization {
            headers.insert(AUTHORIZATION, authorization);
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(60))
            .default_headers(headers)
            .build()
            .map_err(|_| Error::RegistryConfig)?;
        Ok(Self {
            transport: Transport::new(client, destination),
            turn: Arc::new(Semaphore::new(1)),
        })
    }

    pub(super) fn turn(&self) -> Result<OwnedSemaphorePermit, Error> {
        self.turn
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Busy)
    }

    pub(super) fn reference(&self, digest: &str) -> String {
        self.transport.reference(digest)
    }

    pub(super) async fn push(&self, archive: File, plan: &Publication) -> Result<(), Error> {
        tokio::time::timeout(Duration::from_mins(20), async {
            let archive = Arc::new(archive);
            for part in &plan.parts {
                self.transport.push(&archive, part).await?;
            }
            Ok(())
        })
        .await
        .map_err(|_| Error::Registry)?
    }
}

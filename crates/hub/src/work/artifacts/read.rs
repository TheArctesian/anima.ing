//! Open the exact retained attempt under a shared limit for artifact readers.
use std::{fs::File, io, path::Path, sync::Arc};

use futures_util::{Stream, StreamExt as _};
use tokio::sync::OwnedSemaphorePermit;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use super::{Error, Receipt, Store, disk, model, registry::Publisher};
use crate::ids::{DeviceId, JobId};

type ReadLease = (OwnedSemaphorePermit, Store);

pub(super) struct ReadyImage {
    plan: oci_artifact::Publication,
    file: File,
    publisher: Arc<Publisher>,
    _reader: ReadLease,
    _turn: OwnedSemaphorePermit,
}

impl ReadyImage {
    pub(super) fn reference(&self) -> String {
        self.publisher.reference(&self.plan.image.digest)
    }

    pub(super) async fn push(self) -> Result<(), Error> {
        self.publisher.push(self.file, &self.plan).await
    }
}

impl Store {
    pub(super) async fn read(
        &self,
        device: DeviceId,
        job: JobId,
        attempt: u32,
    ) -> Result<
        (
            Receipt,
            impl Stream<Item = io::Result<bytes::Bytes>> + use<>,
        ),
        Error,
    > {
        let (receipt, file, lease) = self
            .retained((device, job, attempt), |path, receipt| {
                disk::verified_file(path, &receipt.artifact)
            })
            .await?;
        let stream = ReaderStream::new(tokio::fs::File::from_std(file)).map(move |part| {
            let _lease = &lease;
            part
        });
        Ok((receipt, stream))
    }

    pub(super) async fn image(
        &self,
        identity: (DeviceId, JobId, u32),
    ) -> Result<(Receipt, oci_artifact::Image), Error> {
        let (receipt, image, _) = self
            .retained(identity, |path, receipt| {
                disk::image(path, &receipt.artifact)
            })
            .await?;
        Ok((receipt, image))
    }

    pub(super) async fn prepare_publication(
        &self,
        identity: (DeviceId, JobId, u32),
        (expected_receipt, expected_digest): (Uuid, String),
    ) -> Result<Option<ReadyImage>, Error> {
        let Some(publisher) = self.1.clone() else {
            return Ok(None);
        };
        let turn = publisher.turn()?;
        let (_, (file, plan), reader) = self
            .retained(identity, move |path, receipt| {
                if expected_receipt != receipt.id
                    || expected_digest != receipt.artifact.manifest_digest
                {
                    return Err(Error::Confirmation);
                }
                disk::publication(path, &receipt.artifact)
            })
            .await?;
        Ok(Some(ReadyImage {
            plan,
            file,
            publisher,
            _reader: reader,
            _turn: turn,
        }))
    }

    async fn retained<T: Send + 'static>(
        &self,
        (device, job, attempt): (DeviceId, JobId, u32),
        read: impl FnOnce(&Path, &Receipt) -> Result<T, Error> + Send + 'static,
    ) -> Result<(Receipt, T, ReadLease), Error> {
        let permit = self
            .0
            .readers
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Busy)?;
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            let path = store
                .0
                .root
                .join(model::attempt_directory(job.into(), attempt));
            let receipt = disk::receipt(&path)?;
            if receipt.device != uuid::Uuid::from(device) {
                return Err(Error::Conflict);
            }
            let contents = read(&path.join("archive.oci"), &receipt)?;
            Ok((receipt, contents, (permit, store)))
        })
        .await
        .map_err(|_| Error::Invalid)?
    }
}

//! Read the exact deployable image metadata of one retained build.
use uuid::Uuid;

use crate::{failure::Failure, invocation::Invocation, output::Table};

#[derive(Debug, clap::Args)]
pub struct ImageArgs {
    /// The source build ID.
    job: Uuid,
    /// The successful attempt number returned by build status.
    #[arg(value_parser = clap::value_parser!(u32).range(1..))]
    attempt: u32,
}

impl ImageArgs {
    pub(super) async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let output = hub.inspect_source_image(self.job, self.attempt).await?;
        invocation.output.data(&output, |output| {
            let mut table = Table::record();
            table.pair("Receipt", output.receipt.to_string());
            table.pair("Image digest", output.image.digest.as_str());
            table.pair("Archive digest", output.archive_digest.as_str());
            table.pair("Media type", output.image.media_type.as_str());
            for platform in &output.image.platforms {
                let mut name = format!("{}/{}", platform.os, platform.architecture);
                if let Some(variant) = &platform.variant {
                    name.push('/');
                    name.push_str(variant);
                }
                table.pair("Platform", name);
            }
            table
        })
    }
}

#[derive(Debug, clap::Args)]
pub struct PublishArgs {
    #[command(flatten)]
    image: ImageArgs,
    /// The receipt shown by inspect-image.
    #[arg(long)]
    receipt: Uuid,
    /// The SHA-256 image digest shown by inspect-image.
    #[arg(long)]
    digest: String,
}

impl PublishArgs {
    pub(super) async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let result = hub
            .publish_source_image(
                (self.image.job, self.image.attempt),
                self.receipt,
                &self.digest,
            )
            .await?;
        invocation.output.data(&result, |result| {
            let mut table = Table::record();
            table.pair("Image", result.reference.as_str());
            table.pair("Receipt", result.receipt.to_string());
            table
        })
    }
}

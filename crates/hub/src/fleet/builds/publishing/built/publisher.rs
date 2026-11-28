//! The publish of a built release, in the publisher's order (BLD-002, BLD-064 to BLD-069); I/O only, the decisions are in `checks`.

use reqwest::Client;

use super::{
    Built, BuiltError,
    checks::{bytes_of, check_built},
    refused,
};
use crate::{
    clock::Clock,
    fleet::builds::{
        model::UploadedFile,
        publishing::{
            bucket::Bucket,
            feed::{self, Expected},
            files,
            run::{self, Reach, Work},
            signature,
            transfer::{self, Stop},
        },
        version::Version,
    },
};

/// `public_key` is the updater public key of the release commit.
pub struct BuiltPublisher<'a> {
    pub bucket: &'a Bucket,
    pub http: &'a Client,
    pub clock: &'a dyn Clock,
    pub public_key: &'a str,
}

impl BuiltPublisher<'_> {
    const fn reach(&self) -> Reach<'_> {
        Reach {
            bucket: self.bucket,
            http: self.http,
            clock: self.clock,
        }
    }

    async fn published(
        &self,
        object: &str,
        (version, feed): (Version, &str),
    ) -> Result<Option<Version>, BuiltError> {
        let reach = self.reach();
        let probe = reach.bucket.head(object, reach.clock.now());
        if transfer::etag_now(reach.http, &probe).await?.is_none() {
            return Ok(None);
        }
        let get = reach.bucket.get(object, reach.clock.now());
        let held = transfer::read_small(reach.http, get, object, feed::MAX_FEED_BYTES).await?;
        let unread = || {
            refused(format!(
                "{object} holds a feed whose version cannot be read."
            ))
        };
        let published = feed::published_version(&held).ok_or_else(unread)?;
        if published == version && held != feed.as_bytes() {
            return Err(refused(format!(
                "{object} already holds another feed of {version}."
            )));
        }
        Ok(Some(published))
    }

    async fn floor(&self, built: &Built, feed: &str) -> Result<Option<Version>, BuiltError> {
        let mut floor = None;
        for folder in feed::platform_keys(built.target) {
            let object = format!("{folder}/latest.json");
            let published = self.published(&object, (built.version, feed)).await?;
            floor = floor.max(published);
        }
        Ok(floor)
    }

    /// A name the bucket already holds with other bytes is refused: a version is never built twice (BLD-002).
    async fn place(&self, built: &Built, file: &UploadedFile) -> Result<String, BuiltError> {
        let reach = self.reach();
        let folder = format!("{}/{}", built.target, built.version);
        let object = format!("{folder}/{}", file.name);
        let probe = reach.bucket.head(&object, reach.clock.now());
        let held = transfer::etag_now(reach.http, &probe).await?.is_some();
        if !held {
            let kind = files::content_type(built.target, built.version, &file.name)
                .ok_or_else(|| refused(format!("{} is not a file of this release.", file.name)))?;
            let bytes = bytes_of(built, &file.name).cloned().unwrap_or_default();
            let upload = reach
                .bucket
                .upload_url(&folder, (file, kind), reach.clock.now());
            transfer::upload(reach.http, upload, bytes).await?;
        }
        let get = reach.bucket.get(&object, reach.clock.now());
        let read = match transfer::read(reach.http, get, file, &mut |_| {}).await {
            Err(Stop::Refused(_)) if held => {
                return Err(refused(format!(
                    "{object} is already in the release bucket with other bytes."
                )));
            }
            other => other?,
        };
        transfer::readable(reach.http, &reach.bucket.public_url(&object), &file.name).await?;
        Ok(read.etag)
    }

    /// Each step can run again: a file the bucket holds with the same bytes is kept, and the same feed is written again.
    ///
    /// # Errors
    /// `Refused` when a check fails, with the feed as it was; `Unavailable` when the bucket does not answer.
    pub async fn publish(&self, built: &Built) -> Result<&'static [&'static str], BuiltError> {
        let reach = self.reach();
        let checked = check_built(built, self.public_key, reach.bucket.public_base())?;
        let feed = feed::signed(&checked.feed, &checked.signature).map_err(refused)?;
        let floor = self.floor(built, &feed).await?;
        let expected = Expected {
            target: built.target,
            version: built.version,
            public_base: reach.bucket.public_base(),
            files: &checked.files,
            floor,
        };
        feed::check(&checked.feed, &expected).map_err(refused)?;
        let (source, stable) = files::stable_copy(built.target, &checked.files).map_err(refused)?;
        let mut source_etag = String::new();
        for file in &checked.files {
            let etag = self.place(built, file).await?;
            if file.name == source {
                source_etag = etag;
            }
        }
        let work = Work {
            target: built.target,
            version: built.version,
            files: checked.files.clone(),
            feed: checked.feed.clone(),
            floor,
            feed_sent: false,
            signed_at: reach.clock.now(),
        };
        signature::write(reach, &work, &checked.updater, &checked.signature).await?;
        let from = format!("{}/{}/{source}", built.target, built.version);
        let copy = reach
            .bucket
            .copy((&from, &source_etag), stable, reach.clock.now());
        transfer::send(reach.http, copy, Vec::new(), stable).await?;
        let folders = feed::platform_keys(built.target);
        for folder in folders {
            run::write_feed(reach, &format!("{folder}/latest.json"), &feed).await?;
        }
        Ok(folders)
    }
}

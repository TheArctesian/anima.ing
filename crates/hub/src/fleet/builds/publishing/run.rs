//! One publish of one job, in the order of BLD-064 to BLD-066; `latest.json` is the last write (UPD-008, D-28).
//! Before each write the try checks it still holds the job, and each step is safe to run again (D-25).
//! Any refusal leaves `latest.json` as it was (BLD-070).

use chrono::{DateTime, TimeDelta, Utc};
use reqwest::Client;
use sqlx::PgPool;
use updater_signature::Prehash;

use super::{
    bucket::{Bucket, NO_CACHE, Write},
    feed::{self, Expected, SUMS},
    files,
    signature::{self, Verified},
    sigv4,
    transfer::{self, Stop},
};
use crate::{
    clock::Clock,
    fleet::builds::{
        model::{Target, UploadedFile},
        store,
        version::Version,
    },
    ids::BuildJobId,
};

pub(super) enum Outcome {
    /// The feed folders written (BLD-067, BLD-068) and the checked files the bucket later held other bytes for (D-28).
    Published(&'static [&'static str], Vec<String>),
    Refused(String),
    Unavailable(&'static str),
    /// A later try took the job, or the hold ran out; this try records nothing (D-25).
    TakenOver,
    Spent,
}

pub(super) struct Work {
    pub(super) target: Target,
    pub(super) version: Version,
    /// The files the upload URLs were given for (D-26).
    pub(super) files: Vec<UploadedFile>,
    pub(super) feed: String,
    /// The highest version the target has published (D-27).
    pub(super) floor: Option<Version>,
    /// An earlier try sent a feed write for the job (D-25).
    pub(super) feed_sent: bool,
    /// When the job reached `uploading`, so the signature's trusted comment is the same on every try (R-117).
    pub(super) signed_at: DateTime<Utc>,
}

#[derive(Clone, Copy)]
pub(super) struct Reach<'a> {
    pub(super) bucket: &'a Bucket,
    pub(super) http: &'a Client,
    pub(super) clock: &'a dyn Clock,
}

/// One try of the publisher on one job, as the fencing token of its writes (D-25).
#[derive(Clone, Copy)]
pub(super) struct Tried {
    pub(super) job: BuildJobId,
    pub(super) attempt: i32,
}

impl Tried {
    /// Stops unless this try still holds its job for as long as a write may take.
    async fn holds(self, db: &PgPool, clock: &dyn Clock) -> Result<(), Stop> {
        let write = TimeDelta::seconds(transfer::WRITE_LIMIT.as_secs().cast_signed());
        match store::still_held(db, self.job, self.attempt, clock.now() + write).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(Stop::TakenOver),
            Err(_) => Err(Stop::Unavailable("database")),
        }
    }
}

pub(super) async fn publish(reach: Reach<'_>, db: &PgPool, tried: Tried, work: &Work) -> Outcome {
    match steps(reach, (db, tried), work).await {
        Ok((folders, changed)) => Outcome::Published(folders, changed),
        Err(Stop::Refused(reason)) => Outcome::Refused(reason),
        Err(Stop::Unavailable(fault)) => Outcome::Unavailable(fault),
        Err(Stop::TakenOver) => Outcome::TakenOver,
    }
}

struct FirstRead {
    sums: Vec<u8>,
    etags: Vec<(String, String)>,
    digest: [u8; 64],
}

/// Hashes the updater file's bytes for its signature as they are checked (BLD-064, D-28).
async fn verify_files(reach: Reach<'_>, work: &Work, updater: &str) -> Result<FirstRead, Stop> {
    let (mut sums, mut etags) = (Vec::new(), Vec::new());
    let mut prehash = Prehash::new();
    for file in &work.files {
        let object = format!("{}/{}/{}", work.target, work.version, file.name);
        let get = reach.bucket.get(&object, reach.clock.now());
        let read = if file.name == updater {
            transfer::read(reach.http, get, file, &mut |chunk| prehash.update(chunk)).await?
        } else {
            transfer::read(reach.http, get, file, &mut |_| {}).await?
        };
        transfer::readable(reach.http, &reach.bucket.public_url(&object), &file.name).await?;
        if file.name == SUMS {
            sums = read.kept;
        }
        etags.push((file.name.clone(), read.etag));
    }
    Ok(FirstRead {
        sums,
        etags,
        digest: prehash.finish(),
    })
}

fn etag_of<'a>(etags: &'a [(String, String)], name: &str) -> &'a str {
    etags
        .iter()
        .find(|(file, _)| file == name)
        .map_or("", |(_, etag)| etag.as_str())
}

async fn steps(
    reach: Reach<'_>,
    (db, tried): (&PgPool, Tried),
    work: &Work,
) -> Result<(&'static [&'static str], Vec<String>), Stop> {
    if work.feed_sent {
        return Ok((confirm(reach, work).await?, Vec::new()));
    }
    let expected = Expected {
        target: work.target,
        version: work.version,
        public_base: reach.bucket.public_base(),
        files: &work.files,
        floor: work.floor,
    };
    let checked = feed::check(&work.feed, &expected).map_err(Stop::Refused)?;
    let (source, stable) = files::stable_copy(work.target, &work.files).map_err(Stop::Refused)?;
    let first = verify_files(reach, work, &checked.updater).await?;
    files::check_sums(&first.sums, &work.files).map_err(Stop::Refused)?;
    let etags = first.etags;
    let updater = work
        .files
        .iter()
        .find(|file| file.name == checked.updater)
        .ok_or_else(|| Stop::Refused("The feed does not name a file of the job.".to_owned()))?;
    let verified = Verified {
        file: updater,
        etag: etag_of(&etags, &checked.updater),
        digest: first.digest,
    };
    let sig = signature::sign(reach, work, &verified).await?;
    tried.holds(db, reach.clock).await?;
    signature::write(reach, work, &checked.updater, &sig).await?;
    let feed = feed::signed(&work.feed, &sig).map_err(Stop::Refused)?;
    tried.holds(db, reach.clock).await?;
    let from = format!("{}/{}/{source}", work.target, work.version);
    let copy = reach
        .bucket
        .copy((&from, etag_of(&etags, &source)), stable, reach.clock.now());
    transfer::send(reach.http, copy, Vec::new(), stable).await?;
    let folders = feed::platform_keys(work.target);
    for folder in folders {
        tried.holds(db, reach.clock).await?;
        store::mark_feed_sent(db, tried.job, reach.clock.now())
            .await
            .map_err(|_| Stop::Unavailable("database"))?;
        write_feed(reach, &format!("{folder}/latest.json"), &feed).await?;
    }
    let watched = [checked.updater.as_str(), SUMS];
    Ok((folders, changed(reach, work, &etags, &watched).await))
}

async fn confirm(reach: Reach<'_>, work: &Work) -> Result<&'static [&'static str], Stop> {
    let folders = feed::platform_keys(work.target);
    let updater = feed::updater_name(work.target, &work.files)
        .ok_or_else(|| Stop::Refused("The job has no single updater file.".to_owned()))?;
    let sig = signature::written(reach, work, updater).await?;
    let feed = feed::signed(&work.feed, &sig).map_err(Stop::Refused)?;
    for folder in folders {
        let object = format!("{folder}/latest.json");
        match feed_in(reach, &object, &feed).await {
            Err(Stop::Refused(_)) => {
                return Err(Stop::Refused(format!(
                    "{object} holds another feed than this job's; its files are kept."
                )));
            }
            other => other?,
        }
    }
    Ok(folders)
}

/// A changed `ETag` means a builder's URL outlived the check (D-28); a failed read logs its fault word only.
async fn changed(
    reach: Reach<'_>,
    work: &Work,
    etags: &[(String, String)],
    watched: &[&str],
) -> Vec<String> {
    let mut changed = Vec::new();
    for name in watched {
        let object = format!("{}/{}/{name}", work.target, work.version);
        let head = reach.bucket.head(&object, reach.clock.now());
        match transfer::etag_now(reach.http, &head).await {
            Ok(now) if now.as_deref() == Some(etag_of(etags, name)) => {}
            Ok(_) => changed.push((*name).to_owned()),
            Err(_) => tracing::warn!(
                domain = "builds",
                fault = "bucket head",
                "publishing could not read a published file's ETag again"
            ),
        }
    }
    changed
}

/// When the bucket's answer is lost, the write may have landed, so the object is read back (D-25).
pub(super) async fn write_feed(reach: Reach<'_>, object: &str, feed: &str) -> Result<(), Stop> {
    let write = Write {
        object,
        headers: vec![
            ("cache-control".to_owned(), NO_CACHE.to_owned()),
            ("content-type".to_owned(), "application/json".to_owned()),
        ],
        body: feed.as_bytes(),
    };
    let put = reach.bucket.put(write, reach.clock.now());
    match transfer::send(reach.http, put, feed.as_bytes().to_vec(), object).await {
        Err(Stop::Unavailable(fault)) if feed_in(reach, object, feed).await.is_err() => {
            Err(Stop::Unavailable(fault))
        }
        Err(Stop::Unavailable(_)) => Ok(()),
        sent => sent,
    }
}

async fn feed_in(reach: Reach<'_>, object: &str, feed: &str) -> Result<(), Stop> {
    let expected = UploadedFile {
        name: object.to_owned(),
        sha256: sigv4::sha256_hex(feed.as_bytes()),
        size: i64::try_from(feed.len()).unwrap_or(i64::MAX),
    };
    let get = reach.bucket.get(object, reach.clock.now());
    transfer::read(reach.http, get, &expected, &mut |_| {})
        .await
        .map(|_| ())
}

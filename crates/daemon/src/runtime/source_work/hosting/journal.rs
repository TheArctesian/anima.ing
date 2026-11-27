//! Keep the start fence and final report across a daemon restart.
use std::{
    fs::{DirBuilder, File},
    io,
    os::unix::fs::DirBuilderExt as _,
    path::PathBuf,
};

use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::wire::{Held, WorkReport, WorkState};
use rustix::fs::{FlockOperation, flock};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::{home::Home, private_file};

use super::reports;

const WINDOW: TimeDelta = TimeDelta::minutes(10);
const MAX_ENTRIES: usize = 128;
pub(super) const MAX_OPEN: usize = 16;
const NAME: &str = "attempts.json";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Stage {
    Reserved,
    Fenced,
    Result {
        report: Box<WorkReport>,
        finishing: bool,
    },
    Settled {
        until: DateTime<Utc>,
    },
}

#[derive(Clone, Serialize, Deserialize)]
struct Attempt {
    device: Uuid,
    origin: String,
    item: Held,
    stage: Stage,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    attempts: Vec<Attempt>,
}

pub(super) struct Journal {
    root: PathBuf,
    lock: File,
    device: Uuid,
    origin: String,
    snapshot: Snapshot,
    failed: bool,
}

impl Journal {
    pub(super) fn open(home: &Home, device: Uuid, origin: String) -> io::Result<Self> {
        home.ensure().map_err(io::Error::other)?;
        let root = home.dir().join("hosting-work");
        match DirBuilder::new().mode(0o700).create(&root) {
            Ok(()) => File::open(home.dir())?.sync_all()?,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error),
        }
        if !private_file::folder_private(&root)? {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let lock = File::open(&root)?;
        flock(&lock, FlockOperation::NonBlockingLockExclusive).map_err(io::Error::from)?;
        let snapshot = match private_file::read(&root.join(NAME)).map_err(io::Error::other)? {
            Some(bytes) => serde_json::from_slice(&bytes)?,
            None => Snapshot::default(),
        };
        let mut journal = Self {
            root,
            lock,
            device,
            origin,
            snapshot,
            failed: false,
        };
        journal.recover(Utc::now())?;
        Ok(journal)
    }

    fn recover(&mut self, now: DateTime<Utc>) -> io::Result<()> {
        let mut next = self.snapshot.clone();
        next.attempts
            .retain(|attempt| self.belongs(attempt) && retained(attempt, now));
        for attempt in &mut next.attempts {
            attempt.stage = match attempt.stage {
                Stage::Reserved => Stage::Settled {
                    until: now + WINDOW,
                },
                Stage::Fenced => Stage::Result {
                    report: Box::new(reports::terminal(
                        attempt.item,
                        WorkState::Failed,
                        Some("orphan"),
                        0,
                    )),
                    finishing: false,
                },
                _ => continue,
            };
        }
        self.save(next)
    }

    fn belongs(&self, attempt: &Attempt) -> bool {
        attempt.device == self.device && attempt.origin == self.origin
    }

    pub(super) fn holds(&self) -> Vec<Held> {
        self.snapshot
            .attempts
            .iter()
            .filter(|attempt| {
                self.belongs(attempt) && !matches!(attempt.stage, Stage::Settled { .. })
            })
            .map(|attempt| attempt.item)
            .collect()
    }

    pub(super) fn has_room(&self, now: DateTime<Utc>) -> bool {
        !self.failed
            && self.holds().len() < MAX_OPEN
            && self
                .snapshot
                .attempts
                .iter()
                .filter(|attempt| retained(attempt, now))
                .count()
                < MAX_ENTRIES
    }

    pub(super) fn reserve(&mut self, item: Held, now: DateTime<Utc>) -> io::Result<()> {
        if !self.has_room(now)
            || self
                .snapshot
                .attempts
                .iter()
                .any(|attempt| self.belongs(attempt) && attempt.item == item)
        {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        let mut next = self.snapshot.clone();
        next.attempts
            .retain(|attempt| retained(attempt, now) && self.belongs(attempt));
        next.attempts.push(Attempt {
            device: self.device,
            origin: self.origin.clone(),
            item,
            stage: Stage::Reserved,
        });
        self.save(next)
    }

    pub(super) fn fence(&mut self, item: Held) -> io::Result<()> {
        self.update(item, |stage| match stage {
            Stage::Reserved => Some(Stage::Fenced),
            _ => None,
        })
    }

    pub(super) fn result(&mut self, report: WorkReport) -> io::Result<()> {
        let item = Held {
            id: report.id,
            attempt: report.attempt,
        };
        self.update(item, |stage| match stage {
            Stage::Reserved | Stage::Fenced => Some(Stage::Result {
                report: Box::new(report),
                finishing: false,
            }),
            _ => None,
        })
    }

    pub(super) fn finished(&mut self, item: Held, report: WorkReport) -> io::Result<()> {
        self.update(item, |stage| match stage {
            Stage::Result {
                finishing: false, ..
            } => Some(Stage::Result {
                report: Box::new(report),
                finishing: true,
            }),
            _ => None,
        })
    }

    pub(super) fn settle(&mut self, item: Held, now: DateTime<Utc>) -> io::Result<()> {
        self.update(item, |_| {
            Some(Stage::Settled {
                until: now + WINDOW,
            })
        })
    }

    pub(super) fn pending(&self) -> impl Iterator<Item = (Held, WorkReport, bool)> + '_ {
        self.snapshot
            .attempts
            .iter()
            .filter(|attempt| !self.failed && self.belongs(attempt))
            .filter_map(|attempt| match &attempt.stage {
                Stage::Result { report, finishing } => {
                    Some((attempt.item, report.as_ref().clone(), *finishing))
                }
                _ => None,
            })
    }

    fn update(
        &mut self,
        item: Held,
        transition: impl FnOnce(&Stage) -> Option<Stage>,
    ) -> io::Result<()> {
        let mut next = self.snapshot.clone();
        let entry = next
            .attempts
            .iter_mut()
            .find(|entry| self.belongs(entry) && entry.item == item)
            .ok_or(io::ErrorKind::NotFound)?;
        entry.stage = transition(&entry.stage).ok_or(io::ErrorKind::InvalidData)?;
        self.save(next)
    }

    fn save(&mut self, next: Snapshot) -> io::Result<()> {
        if self.failed {
            return Err(io::ErrorKind::Other.into());
        }
        let bytes = serde_json::to_vec(&next)?;
        if next.attempts.len() > MAX_ENTRIES || bytes.len() > private_file::LIMIT {
            return Err(io::ErrorKind::FileTooLarge.into());
        }
        self.failed = true;
        private_file::write(&self.root, NAME, &bytes).map_err(io::Error::other)?;
        self.failed = false;
        self.snapshot = next;
        Ok(())
    }
}

impl Drop for Journal {
    fn drop(&mut self) {
        let _ = flock(&self.lock, FlockOperation::Unlock);
    }
}

fn retained(attempt: &Attempt, now: DateTime<Utc>) -> bool {
    !matches!(attempt.stage, Stage::Settled { until } if until <= now)
}

mod journal;
pub mod schema;

use super::probe;
use journal::Journal;
use schema::{Action, Failure, Operation, Outcome, Record, State};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct Jobs {
    journal: Arc<Mutex<Journal>>,
    program: PathBuf,
    boot_id: String,
    admissions: Arc<tokio::sync::Semaphore>,
}

impl Jobs {
    pub fn open(directory: &Path, program: &Path) -> Result<Self, Failure> {
        let boot_id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .map_err(|_| Failure::JournalUnavailable)?
            .trim()
            .to_owned();
        Ok(Self {
            journal: Arc::new(Mutex::new(Journal::open(directory)?)),
            program: program.into(),
            boot_id,
            admissions: Arc::new(tokio::sync::Semaphore::new(2)),
        })
    }

    pub async fn request(&self, operation: Operation) -> Result<Outcome, Failure> {
        let permit = self
            .admissions
            .clone()
            .try_acquire_owned()
            .map_err(|_| Failure::Busy)?;
        let jobs = self.clone();
        let task = tokio::spawn(async move {
            let worker = jobs.clone();
            let (outcome, start) = tokio::task::spawn_blocking(move || worker.dispatch(operation))
                .await
                .map_err(|_| Failure::JournalUnavailable)??;
            if let Some(id) = start {
                jobs.spawn(id);
            }
            drop(permit);
            Ok(outcome)
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), task)
            .await
            .map_err(|_| Failure::TimedOut)?
            .map_err(|_| Failure::JournalUnavailable)?
    }

    fn spawn(self, id: Uuid) {
        tokio::spawn(async move {
            if let Err(error) = self.run(id).await {
                eprintln!("anima-osd: job failure={error}");
            }
        });
    }

    fn dispatch(&self, operation: Operation) -> Result<(Outcome, Option<Uuid>), Failure> {
        let mut journal = self
            .journal
            .lock()
            .map_err(|_| Failure::JournalUnavailable)?;
        journal.prune()?;
        let mut start = None;
        let outcome = match operation {
            Operation::Start { request_id, action } => {
                let (record, created) = self.accept(&mut journal, request_id, action)?;
                start = created.then_some(record.id);
                Outcome::Operation(record)
            }
            Operation::Get { request_id } => Outcome::Operation(
                journal
                    .records
                    .values()
                    .find(|r| r.request_id == request_id)
                    .cloned()
                    .ok_or(Failure::NotFound)?,
            ),
            Operation::Cancel { operation_id } => {
                let mut record = journal
                    .records
                    .get(&operation_id)
                    .cloned()
                    .ok_or(Failure::NotFound)?;
                if record.state == State::Cancelled {
                    return Ok((Outcome::Operation(record), None));
                }
                if record.state.terminal() {
                    return Err(Failure::CancelNotAllowed);
                }
                record.cancel_requested = true;
                if record.state == State::Queued {
                    record.state = State::Cancelled;
                }
                Outcome::Operation(journal.save(record)?)
            }
            Operation::List { after } => {
                let records: Vec<_> = journal
                    .records
                    .values()
                    .filter(|r| after.is_none_or(|id| r.id > id))
                    .take(33)
                    .cloned()
                    .collect();
                let next = (records.len() > 32).then(|| records[31].id);
                Outcome::Operations {
                    records: records.into_iter().take(32).collect(),
                    next,
                }
            }
        };
        Ok((outcome, start))
    }

    fn accept(
        &self,
        journal: &mut Journal,
        request_id: Uuid,
        action: Action,
    ) -> Result<(Record, bool), Failure> {
        if request_id.is_nil() {
            return Err(Failure::InvalidRequest);
        }
        if let Some(record) = journal
            .records
            .values()
            .find(|r| r.request_id == request_id)
        {
            return Ok((record.clone(), false));
        }
        if journal
            .records
            .values()
            .filter(|r| !r.state.terminal())
            .count()
            >= 8
        {
            return Err(Failure::Busy);
        }
        Ok((journal.save(self.new_record(request_id, action))?, true))
    }

    fn new_record(&self, request_id: Uuid, action: Action) -> Record {
        Record {
            id: Uuid::now_v7(),
            request_id,
            action,
            state: State::Queued,
            revision: 0,
            cancel_requested: false,
            created_seconds: journal::seconds(),
            updated_seconds: 0,
            boot_id: self.boot_id.clone(),
            result: None,
        }
    }

    async fn run(&self, id: Uuid) -> Result<(), Failure> {
        let journal = self.journal.clone();
        let run = tokio::task::spawn_blocking(move || {
            let mut journal = journal.lock().map_err(|_| Failure::JournalUnavailable)?;
            let mut record = journal.records.get(&id).cloned().ok_or(Failure::NotFound)?;
            if record.state.terminal() {
                return Ok(false);
            }
            record.state = State::Running;
            journal.save(record)?;
            Ok(true)
        })
        .await
        .map_err(|_| Failure::JournalUnavailable)??;
        if !run {
            return Ok(());
        }
        let result = probe::read(&self.program).await;
        let journal = self.journal.clone();
        tokio::task::spawn_blocking(move || {
            let mut journal = journal.lock().map_err(|_| Failure::JournalUnavailable)?;
            let mut record = journal.records.get(&id).cloned().ok_or(Failure::NotFound)?;
            record.state = if record.cancel_requested {
                State::Cancelled
            } else {
                State::Succeeded
            };
            record.result = (!record.cancel_requested).then_some(result);
            journal.save(record)?;
            journal.prune()
        })
        .await
        .map_err(|_| Failure::JournalUnavailable)?
    }
}

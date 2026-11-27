//! Execute checked application plans on a Linux Docker host.
pub(crate) mod files;
pub(crate) mod hosting;
pub(crate) mod process;
#[cfg(test)]
mod tests;
pub(crate) mod version;

use std::{
    fs, io,
    os::unix::fs::{FileTypeExt as _, MetadataExt as _},
    path::Path,
    time::Instant,
};

use crate::{Action, Paths, Plan, Step};
use anima_os_maintenance::hosting::Operation;
use files::Store;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Fault {
    #[error("the application operation reached its time limit")]
    LimitSeconds,
    #[error("the application plan names an unsafe path or command")]
    UnsafePlan,
    #[error("the Docker socket is missing or its ownership has changed")]
    Socket,
    #[error("the application store could not complete an operation ({0})")]
    Storage(io::ErrorKind),
    #[error("the application command could not start ({0})")]
    Spawn(io::ErrorKind),
    #[error("the application command could not be supervised ({0})")]
    Process(io::ErrorKind),
    #[error("the application command failed")]
    CommandFailed,
    #[error("the application command succeeded but its version records are unconfirmed")]
    VersionUnconfirmed,
    #[error("the imported application requires a verified local cutover")]
    MigrationHeld,
    #[error("maintenance or another command holds the hosting dispatch")]
    HostingBusy,
    #[error("the Docker command outcome requires local reconciliation")]
    HostingUnconfirmed,
}

impl From<io::Error> for Fault {
    fn from(error: io::Error) -> Self {
        Self::Storage(error.kind())
    }
}

impl From<rustix::io::Errno> for Fault {
    fn from(error: rustix::io::Errno) -> Self {
        io::Error::from(error).into()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepFailure {
    pub index: usize,
    pub fault: Fault,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub failed: Option<StepFailure>,
    pub attempts: Vec<StepFailure>,
    pub cleanup: Vec<StepFailure>,
}

pub struct Executor<'a> {
    paths: &'a Paths,
    store: Store<'a>,
}

impl<'a> Executor<'a> {
    /// Open the private application directory and hold its exclusive execution lock.
    /// Paths come from the administrator's startup checks.
    ///
    /// # Errors
    /// Unsafe paths, a directory owned by another executor, or inaccessible storage.
    pub fn open(paths: &'a Paths) -> Result<Self, Fault> {
        for path in [
            &paths.docker,
            &paths.compose,
            &paths.configuration,
            &paths.socket,
        ] {
            if !Path::new(path).is_absolute() {
                return Err(Fault::UnsafePlan);
            }
        }
        check_socket(paths)?;
        Ok(Self {
            paths,
            store: Store::open(Path::new(&paths.root))?,
        })
    }

    pub(crate) const fn store(&self) -> &Store<'a> {
        &self.store
    }

    /// Run a plan emitted by the application owner. One deadline covers the entire plan.
    /// Docker command output stays private. Follow-mode output has its own transport.
    #[must_use]
    pub fn run(&mut self, plan: &Plan, deadline: Instant) -> Report {
        self.run_while(plan, deadline, || Ok(()))
    }

    pub(crate) fn run_while(
        &mut self,
        plan: &Plan,
        deadline: Instant,
        permitted: impl Fn() -> Result<(), Fault>,
    ) -> Report {
        let mut report = Report::default();
        if let Err(fault) = check_socket(self.paths) {
            report.failed = Some(StepFailure { index: 0, fault });
            return report;
        }
        for (index, step) in plan.steps.iter().enumerate() {
            if let Step::DeleteFolderInEachCase { path } = step {
                if let Err(fault) = self.store.delete_folder(path) {
                    let failure = StepFailure { index, fault };
                    report.cleanup.push(failure);
                    report.failed.get_or_insert(failure);
                }
                continue;
            }
            if report.failed.is_some() {
                continue;
            }
            let allowed = if matches!(step, Step::Run(_) | Step::Attempt(_)) {
                permitted()
            } else {
                Ok(())
            };
            if let Err(fault) = allowed.and_then(|()| self.step(step, deadline, &permitted)) {
                let failure = StepFailure { index, fault };
                if matches!(step, Step::Attempt(_))
                    && matches!(fault, Fault::CommandFailed | Fault::Spawn(_))
                {
                    report.attempts.push(failure);
                } else {
                    report.failed = Some(failure);
                }
            }
        }
        report
    }

    fn step(
        &self,
        step: &Step,
        deadline: Instant,
        permitted: impl Fn() -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        if Instant::now() >= deadline {
            return Err(Fault::LimitSeconds);
        }
        match step {
            Step::Folder { path } => self.store.folder(path),
            Step::Write { path, content } => self.store.write(path, content),
            Step::Delete { path } => self.store.delete_file(path),
            Step::DeleteFolder { path } => self.store.delete_folder(path),
            Step::Absent { path } => self.store.absent(path),
            Step::RealFolder { path, below } => self.store.real_folder(path, below),
            Step::BindFolder { path } => self.store.bind_folder(path),
            Step::Run(command) | Step::Attempt(command) => {
                check_socket(self.paths)?;
                process::run(self.paths, command, deadline)
            }
            Step::ApplyVersion { command, version } => version::apply(&self.store, version, || {
                permitted()?;
                crate::migration::check_start(
                    &self.store,
                    (&version.folders, &self.paths.socket),
                    deadline,
                )?;
                check_socket(self.paths)?;
                if Instant::now() >= deadline {
                    return Err(Fault::LimitSeconds);
                }
                hosting::run(self.paths, Operation::Apply, command, deadline)
            }),
            Step::DeleteFolderInEachCase { .. } => Err(Fault::UnsafePlan),
        }
    }
}

pub(crate) fn check_socket(paths: &Paths) -> Result<(), Fault> {
    let metadata = fs::symlink_metadata(&paths.socket).map_err(|_| Fault::Socket)?;
    if metadata.file_type().is_socket() && metadata.uid() == rustix::process::geteuid().as_raw() {
        Ok(())
    } else {
        Err(Fault::Socket)
    }
}

pub(crate) fn run_container(
    paths: &Paths,
    plan: &Plan,
    action: Action,
    deadline: Instant,
) -> Result<(), Fault> {
    if Instant::now() >= deadline {
        return Err(Fault::LimitSeconds);
    }
    let [Step::Run(command)] = plan.steps.as_slice() else {
        return Err(Fault::UnsafePlan);
    };
    check_socket(paths)?;
    match action {
        Action::Stop => process::run(paths, command, deadline),
        Action::Start => hosting::run(paths, Operation::Start, command, deadline),
        Action::Restart => hosting::run(paths, Operation::Restart, command, deadline),
    }
}

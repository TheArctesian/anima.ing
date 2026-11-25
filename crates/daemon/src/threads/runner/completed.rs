//! Results of background discovery and project creation.

use agent_threads::drive::Program;
use tokio::sync::mpsc;

use super::{Runner, git_reads, models, projects};

pub(super) enum Completed {
    Models(Program, models::Listed),
    Project(projects::Created),
    GitRead(Box<git_reads::Completed>),
}

pub(super) async fn next(
    models: &mut mpsc::Receiver<(Program, models::Listed)>,
    projects: &mut mpsc::Receiver<projects::Created>,
    git_reads: &mut mpsc::Receiver<git_reads::Completed>,
) -> Completed {
    tokio::select! {
        Some((program, listed)) = models.recv() => Completed::Models(program, listed),
        Some(created) = projects.recv() => Completed::Project(created),
        Some(read) = git_reads.recv() => Completed::GitRead(Box::new(read)),
    }
}

impl Runner {
    pub(super) fn completed(&mut self, result: Completed) {
        match result {
            Completed::Models(program, listed) => self.models_asked(program, listed),
            Completed::Project(created) => self.project_created(created),
            Completed::GitRead(read) => self.git_read_completed(*read),
        }
    }
}

//! Results of background discovery and project creation.

use agent_threads::drive::Program;
use tokio::sync::mpsc;

use super::{Runner, models, projects};

pub(super) enum Completed {
    Models(Program, models::Listed),
    Project(projects::Created),
}

pub(super) async fn next(
    models: &mut mpsc::Receiver<(Program, models::Listed)>,
    projects: &mut mpsc::Receiver<projects::Created>,
) -> Completed {
    tokio::select! {
        Some((program, listed)) = models.recv() => Completed::Models(program, listed),
        Some(created) = projects.recv() => Completed::Project(created),
    }
}

impl Runner {
    pub(super) fn completed(&mut self, result: Completed) {
        match result {
            Completed::Models(program, listed) => self.models_asked(program, listed),
            Completed::Project(created) => self.project_created(created),
        }
    }
}

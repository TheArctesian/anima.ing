//! Creation of local Git projects for the person (TH60A).

use std::path::PathBuf;

use agent_threads::control::{Outcome, ProjectChange, Refusal, project_changed};
use tokio::sync::mpsc;

use super::Runner;
use super::socket::Caller;

mod create;

pub(super) struct Created {
    back: Caller,
    result: Result<ProjectChange, Refusal>,
}

pub(super) struct Creations {
    sender: mpsc::Sender<Created>,
    pub receiver: Option<mpsc::Receiver<Created>>,
}

impl Default for Creations {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel(3);
        Self {
            sender,
            receiver: Some(receiver),
        }
    }
}

static CREATING: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(3);

impl Runner {
    pub(super) fn create_project(&self, folder: PathBuf, name: Option<String>, back: Caller) {
        let change = ProjectChange::Add {
            folder: folder.clone(),
            name: name.clone(),
        };
        if project_changed(self.projects.clone(), &change, folder.clone()).is_err() {
            back.answer(Outcome::Refused {
                why: refused("The project name or project list does not permit this addition."),
            });
            return;
        }
        let Ok(place) = CREATING.try_acquire() else {
            back.answer(Outcome::Refused {
                why: refused("Other projects are being created. Try again when they finish."),
            });
            return;
        };
        let daemon = self.dir.parent().unwrap_or(&self.dir).to_owned();
        let sender = self.project_creations.sender.clone();
        tokio::spawn(async move {
            let _place = place;
            let result = create::project(folder, daemon)
                .await
                .map(|folder| ProjectChange::Add { folder, name });
            let _ = sender.send(Created { back, result }).await;
        });
    }

    pub(super) fn project_created(&mut self, created: Created) {
        let result = created.result.and_then(|change| {
            let projects = project_changed(self.projects.clone(), &change, change.folder().clone())
                .map_err(|_| refused("The folder was created, but the project list changed. Add the folder as a local project."))?;
            let before = std::mem::replace(&mut self.projects, projects);
            if !self.written() {
                self.projects = before;
                return Err(refused("The folder was created, but its registration could not be saved. Add the folder as a local project."));
            }
            Ok(())
        });
        let answer = match result {
            Ok(()) => self.projects(),
            Err(why) => Outcome::Refused { why },
        };
        self.answer_call(&created.back, None, answer);
    }
}

fn refused(said: &str) -> Refusal {
    Refusal::CannotCreateProject {
        said: said.to_owned(),
    }
}

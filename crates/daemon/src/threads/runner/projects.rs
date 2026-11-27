//! Preparation of local Git projects for the person (TH60A, TH60B).

use agent_threads::control::{Action, Outcome, ProjectChange, Refusal, project_changed};
use tokio::sync::SemaphorePermit;
use tokio::task::JoinSet;

use super::Runner;
use super::socket::Caller;

mod clone;
mod create;

pub(super) struct Created {
    back: Caller,
    result: Result<create::Prepared, Refusal>,
    name: Option<String>,
    _place: SemaphorePermit<'static>,
}

pub(super) type Creations = JoinSet<Created>;

static CREATING: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(3);

impl Runner {
    pub(super) fn prepare_project(&mut self, action: Action, back: Caller) {
        let (folder, name, preparation) = match action {
            Action::CreateProject { folder, name } => {
                (folder, name, create::Preparation::Initialize)
            }
            Action::CloneProject { folder, name, url } => match clone::Source::parse(&url) {
                Ok(source) => (folder, name, create::Preparation::Clone(source)),
                Err(why) => return back.answer(Outcome::Refused { why }),
            },
            _ => {
                return back.answer(Outcome::Refused {
                    why: Refusal::BadValue,
                });
            }
        };
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
        self.project_creations.spawn(async move {
            let result = create::project(folder, daemon, preparation).await;
            Created {
                back,
                result,
                name,
                _place: place,
            }
        });
    }

    pub(super) fn project_created(&mut self, created: Created) {
        let result = created.result.and_then(|prepared| {
            // Only the runner commits a prepared result. A worker cancelled or discarded
            // at shutdown owns private staging and cannot publish a project on its own.
            let project = prepared.publish()?;
            let change = ProjectChange::Add {
                folder: project.folder()?,
                name: created.name,
            };
            let projects = project_changed(self.projects.clone(), &change, change.folder().clone())
                .map_err(|_| {
                    refused(
                        "The folder was created, but the project list changed before registration.",
                    )
                })?;
            let before = std::mem::replace(&mut self.projects, projects);
            if !self.written() {
                self.projects = before;
                return Err(refused(
                    "The folder was created, but its registration could not be saved.",
                ));
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

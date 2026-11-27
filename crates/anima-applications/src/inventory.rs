//! Which containers and projects a type reaches, from values the caller gives (contract K10 Section 6.4.3).

use crate::{
    Refusal, Settings,
    forms::{is_engine_name, is_lower_hex, project_of},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Container {
    /// 64 lower-case hexadecimal characters.
    pub id: String,
    /// The name, with no `/` before it.
    pub name: String,
    /// The label `com.docker.compose.project`.
    pub project: Option<String>,
    /// The label `com.docker.compose.project.working_dir`.
    pub folder: Option<String>,
    pub running: bool,
    /// True when the restart policy is not `no`.
    pub restarts: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    pub name: String,
    /// The label `com.docker.compose.project`.
    pub project: Option<String>,
    /// True when a container uses it.
    pub used: bool,
    /// True for a network that the engine made itself (`bridge`, `host`, `none`).
    pub of_the_engine: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub name: String,
    /// The label `com.docker.compose.project`.
    pub project: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// The identifier: `sha256:` and 64 hexadecimal characters.
    pub id: String,
    /// Each name by which a registry gave it, as `name@sha256:…`.
    pub pulled_as: Vec<String>,
    /// True when a container uses it, running or not.
    pub used: bool,
    /// True when it has no tag (what `docker image prune` removes).
    pub dangling: bool,
}

/// What the engine has at the time of one item.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    pub containers: Vec<Container>,
    pub networks: Vec<Network>,
    pub volumes: Vec<Volume>,
    pub images: Vec<Image>,
}

/// The digest by which the program pulled the image of one service.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pulled {
    pub service: String,
    /// The image as the file named it.
    pub image: String,
    /// `sha256:` and 64 hexadecimal characters.
    pub digest: String,
}

/// The record of one application that Anima installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installed {
    /// The UUID of the application.
    pub application: String,
    /// True when `files/` of the application exists with its final form.
    pub has_files: bool,
    /// The digests of the last apply.
    pub current: Vec<Pulled>,
    /// The digests of the kept version; `None` when no version is kept.
    pub kept: Option<Vec<Pulled>>,
    /// Each image pulled for it and not removed, as `name@sha256:…`.
    pub pulled: Vec<String>,
}

/// The records of the program: the applications that Anima installed on this machine.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Records {
    pub applications: Vec<Installed>,
}

impl Records {
    #[must_use]
    pub fn installed(&self, application: &str) -> Option<&Installed> {
        self.applications
            .iter()
            .find(|installed| installed.application == application)
    }

    pub(crate) fn holds_digest(&self, digest: &str, except: Option<&str>) -> bool {
        self.applications
            .iter()
            .filter(|installed| Some(installed.application.as_str()) != except)
            .flat_map(|installed| {
                installed
                    .current
                    .iter()
                    .chain(installed.kept.iter().flatten())
            })
            .any(|one| one.digest == digest)
    }

    pub(crate) fn has_project(&self, project: &str) -> bool {
        self.applications
            .iter()
            .any(|installed| project_of(&installed.application) == project)
    }
}

/// The name part is not compared: the engine and a file can write one image with two names.
pub(crate) fn digest_of(pulled: &str) -> Option<&str> {
    pulled.rsplit_once('@').map(|(_, digest)| digest)
}

impl Container {
    /// True when the identifier and the name have the forms that go into a list of arguments.
    pub(crate) fn is_sound(&self) -> bool {
        is_lower_hex(&self.id, 64) && is_engine_name(&self.name)
    }

    pub(crate) fn is_of(&self, project: &str) -> bool {
        self.project.as_deref() == Some(project)
    }
}

pub(crate) fn is_installed(container: &Container, records: &Records) -> bool {
    container
        .project
        .as_deref()
        .is_some_and(|project| records.has_project(project))
}

fn is_adopted(container: &Container, settings: &Settings) -> bool {
    container
        .project
        .as_deref()
        .is_some_and(|project| settings.adopted_project(project).is_some())
}

pub(crate) fn project_is_protected(name: &str, settings: &Settings, inventory: &Inventory) -> bool {
    settings.protects(name)
        || inventory
            .containers
            .iter()
            .any(|container| container.is_of(name) && settings.protects(&container.name))
}

pub(crate) fn container_is_protected(container: &Container, settings: &Settings) -> bool {
    settings.protects(&container.name)
        || container
            .project
            .as_deref()
            .is_some_and(|project| settings.protects(project))
}

pub(crate) fn reached<'a>(
    id: &str,
    settings: &Settings,
    inventory: &'a Inventory,
    records: &Records,
) -> Result<&'a Container, Refusal> {
    let container = inventory
        .containers
        .iter()
        .find(|container| container.id == id && container.is_sound())
        .ok_or(Refusal::NoContainer)?;
    if container_is_protected(container, settings) {
        return Err(Refusal::Protected);
    }
    if settings.all_containers
        || is_installed(container, records)
        || is_adopted(container, settings)
    {
        Ok(container)
    } else {
        Err(Refusal::NotAdopted)
    }
}

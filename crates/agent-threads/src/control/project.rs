//! The projects of this computer (TH107): folders that the person adds, each with a name, so
//! that the threads of a folder are listed under it. Kept in the record of the runner only.
//! Pure: the runner checks that the folder is a folder of this computer.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::listing::{BadTitle, checked_title};

/// The most projects that the record keeps.
pub const MAX_PROJECTS: usize = 500;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// The folder as the runner found it: absolute, with no link in it.
    pub folder: PathBuf,
    /// The name the person gave; by default the last part of the folder.
    pub name: String,
}

/// A change of the projects by the person. A change names a project by its folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum ProjectChange {
    /// Adds a folder; with no name, the last part of the folder is the name.
    Add {
        folder: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    Rename {
        folder: PathBuf,
        name: String,
    },
    /// Takes the project off the list: no file and no thread of it changes.
    Remove {
        folder: PathBuf,
    },
}

/// Why a change of the projects is not kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BadProject {
    #[error(transparent)]
    Name(#[from] BadTitle),
    #[error("the folder is a project already")]
    Known,
    #[error("the folder is not a project")]
    Unknown,
    #[error("the list holds {MAX_PROJECTS} projects already")]
    Full,
}

/// `projects` after `change`, with `folder` the folder of the change as the runner found it.
/// The list stays in the order the person added the projects.
///
/// # Errors
/// [`BadProject`] for a name that is not a title (THR-125), a folder added twice, a folder that
/// is not a project, or a full list.
pub fn changed(
    mut projects: Vec<Project>,
    change: &ProjectChange,
    folder: PathBuf,
) -> Result<Vec<Project>, BadProject> {
    let at = projects.iter().position(|project| project.folder == folder);
    match (change, at) {
        (ProjectChange::Add { .. }, Some(_)) => return Err(BadProject::Known),
        (ProjectChange::Add { name, .. }, None) => {
            if projects.len() >= MAX_PROJECTS {
                return Err(BadProject::Full);
            }
            let name = match name {
                Some(name) => checked_title(name)?,
                None => checked_title(&last_part(&folder))?,
            };
            projects.push(Project { folder, name });
        }
        (ProjectChange::Rename { name, .. }, Some(at)) => projects[at].name = checked_title(name)?,
        (ProjectChange::Remove { .. }, Some(at)) => {
            projects.remove(at);
        }
        (ProjectChange::Rename { .. } | ProjectChange::Remove { .. }, None) => {
            return Err(BadProject::Unknown);
        }
    }
    Ok(projects)
}

impl ProjectChange {
    /// The folder that the change names.
    #[must_use]
    pub const fn folder(&self) -> &PathBuf {
        match self {
            Self::Add { folder, .. } | Self::Rename { folder, .. } | Self::Remove { folder } => {
                folder
            }
        }
    }
}

fn last_part(folder: &std::path::Path) -> String {
    folder.file_name().map_or_else(
        || folder.to_string_lossy().into_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(folder: &str, name: Option<&str>) -> ProjectChange {
        ProjectChange::Add {
            folder: folder.into(),
            name: name.map(str::to_owned),
        }
    }

    fn apply(projects: Vec<Project>, change: &ProjectChange) -> Result<Vec<Project>, BadProject> {
        changed(projects, change, change.folder().clone())
    }

    #[test]
    fn th107_a_project_takes_the_last_part_of_its_folder_as_its_name_and_keeps_its_place() {
        let one = apply(Vec::new(), &add("/w/anima", None)).expect("added");
        let two = apply(one, &add("/w/site", Some("  The site  "))).expect("added");
        let names: Vec<&str> = two.iter().map(|project| project.name.as_str()).collect();
        assert_eq!(names, ["anima", "The site"]);
        let renamed = apply(
            two,
            &ProjectChange::Rename {
                folder: "/w/anima".into(),
                name: "Anima".to_owned(),
            },
        )
        .expect("renamed");
        assert_eq!(renamed[0].name, "Anima");
        let left = apply(
            renamed,
            &ProjectChange::Remove {
                folder: "/w/anima".into(),
            },
        )
        .expect("removed");
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].folder, PathBuf::from("/w/site"));
    }

    #[test]
    fn th107_a_folder_twice_an_unknown_folder_and_a_hidden_name_are_refused() {
        let one = apply(Vec::new(), &add("/w/anima", None)).expect("added");
        assert_eq!(
            apply(one.clone(), &add("/w/anima", None)),
            Err(BadProject::Known)
        );
        let unknown = ProjectChange::Remove {
            folder: "/w/none".into(),
        };
        assert_eq!(apply(one.clone(), &unknown), Err(BadProject::Unknown));
        assert_eq!(
            apply(one, &add("/w/b", Some("pay\u{202E}evil"))),
            Err(BadProject::Name(BadTitle))
        );
    }

    #[test]
    fn th107_the_list_has_a_bound() {
        let full: Vec<Project> = (0..MAX_PROJECTS)
            .map(|at| Project {
                folder: format!("/w/{at}").into(),
                name: at.to_string(),
            })
            .collect();
        assert_eq!(apply(full, &add("/w/more", None)), Err(BadProject::Full));
    }
}

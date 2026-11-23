//! A repository by `owner/name` (PRJ-016) or by ID.

use super::documents::{ProjectList, project_list};
use crate::failure::{Failure, Hint};
use crate::hub::Hub;
use crate::paging::{MAX_PAGE, fetch};

struct Match {
    id: String,
    left_out: bool,
    gone: bool,
}

impl Match {
    fn shown(&self) -> String {
        let state = match (self.left_out, self.gone) {
            (true, true) => "left out, gone",
            (true, false) => "left out",
            (false, true) => "gone",
            (false, false) => "included",
        };
        format!("{} ({state})", self.id)
    }
}

async fn matches(hub: &Hub, name: &str) -> Result<Vec<Match>, Failure> {
    let mut found: Vec<Match> = Vec::new();
    for left_out in [false, true] {
        let projects = fetch(None, true, move |after| async move {
            let variables = project_list::Variables {
                left_out: Some(left_out),
                first: Some(MAX_PAGE),
                after,
                browse: None,
            };
            Ok(hub.run::<ProjectList>(variables).await?.projects)
        })
        .await?;
        let named = projects
            .edges
            .iter()
            .flat_map(|edge| &edge.node.repositories)
            .filter(|repository| repository.full_name == name);
        for repository in named {
            if !found.iter().any(|seen| seen.id == repository.id) {
                found.push(Match {
                    id: repository.id.clone(),
                    left_out: repository.left_out,
                    gone: repository.gone,
                });
            }
        }
    }
    Ok(found)
}

/// A left-out or gone repository keeps its old name, so a shared name is refused, not guessed.
///
/// # Errors
/// [`Failure::NotFound`] when no project holds the name; [`Failure::Usage`] when several do.
pub async fn resolve(hub: &Hub, name: &str) -> Result<String, Failure> {
    if !name.contains('/') {
        return Ok(name.to_owned());
    }
    let mut found = matches(hub, name).await?;
    match found.len() {
        0 => Err(Failure::NotFound {
            message: format!("No project has a repository named {name}."),
            hint: Some(Hint::run("anima project list --all")),
        }),
        1 => Ok(found.remove(0).id),
        count => {
            let listed: Vec<String> = found.iter().map(Match::shown).collect();
            Err(Failure::Usage {
                message: format!(
                    "{count} repositories are named {name}: {}. Give the ID.",
                    listed.join(", ")
                ),
                hint: Some(Hint::run("anima project list --all --json")),
            })
        }
    }
}

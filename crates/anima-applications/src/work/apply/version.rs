//! Build the checked command and private records for an apply or rollback.
use crate::{
    Folders, Plan, Pulled, Refusal, Step,
    forms::project_of,
    plan::{Project, Version},
};

use super::{
    Facts, Planned, Prepared, final_form_of,
    records::{MAX_PULLED_BYTES, digests_written, pulled_written},
    sha256,
};

pub(crate) fn plan_version(
    prepared: &Prepared,
    digests: &[Pulled],
    facts: &Facts<'_>,
    record_pulls: bool,
) -> Result<Planned, Refusal> {
    let form = final_form_of(prepared, digests, facts)?;
    let folders = facts.paths.application(&prepared.application);
    let mut steps = folder_steps(&folders);
    if record_pulls {
        // The pulled images are present even if Compose fails.
        let pulled = pulled_written(prepared, digests, facts);
        if pulled.len() as u64 > MAX_PULLED_BYTES {
            return Err(Refusal::LimitDisk);
        }
        steps.push(Step::Write {
            path: folders.pulled(),
            content: pulled,
        });
    }
    steps.extend(boundary_checks(&folders, &form.binds));

    let command = start_command(facts, &folders, &prepared.application);
    let keep_previous = prepared.keep
        && facts
            .records
            .installed(&prepared.application)
            .is_some_and(|installed| installed.has_files);
    let final_form_sha256 = sha256(form.text.as_bytes());
    steps.push(Step::ApplyVersion {
        command,
        version: Box::new(Version {
            folders,
            keep_previous,
            compose: prepared.compose.clone(),
            environment: prepared.environment.clone(),
            digests: digests_written(digests),
            final_form: form.text.into_bytes(),
        }),
    });
    Ok(Planned {
        plan: Plan { steps },
        final_form_sha256,
        holds_volumes: form.holds_volumes,
    })
}

fn folder_steps(folders: &Folders) -> Vec<Step> {
    [
        folders.application().to_owned(),
        folders.files(),
        folders.binds(),
    ]
    .into_iter()
    .map(|path| Step::Folder { path })
    .collect()
}

fn boundary_checks(folders: &Folders, binds: &[String]) -> Vec<Step> {
    let mut steps = vec![
        Step::Absent {
            path: folders.dot_env(),
        },
        Step::RealFolder {
            path: folders.binds(),
            below: folders.application().to_owned(),
        },
    ];
    steps.extend(binds.iter().map(|name| Step::BindFolder {
        path: format!("{}/{name}", folders.binds()),
    }));
    steps
}

fn start_command(facts: &Facts<'_>, folders: &Folders, application: &str) -> crate::Command {
    let project = project_of(application);
    let files = [folders.final_form()];
    let named = Project {
        name: &project,
        folder: &folders.files(),
        files: &files,
    };
    let up = [
        "up",
        "-d",
        "--remove-orphans",
        "--pull",
        "never",
        "--no-build",
    ];
    facts.paths.compose(&named, &up)
}

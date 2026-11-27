//! `application_apply` (K10 Section 6.4.6). The check has three parts, because the program gets
//! the files and pulls the images between them.

mod records;
mod version;

use std::fmt::Write;
use zeroize::Zeroize as _;

use compose_rules::{Checked, Node, Rule};
use sha2::{Digest, Sha256};

use super::{Facts, free, managed};
use crate::{
    Apply, Command, FinalForm, Frame, Plan, Pulled, Refusal, final_form, forms::project_of,
    inventory::project_is_protected,
};

#[cfg(target_os = "linux")]
pub(crate) use records::MAX_PULLED_BYTES;
pub(crate) use version::plan_version;

/// The two files of an item; `{:?}` shows only their lengths.
#[derive(Clone, Copy)]
pub struct Files<'a> {
    pub compose: &'a [u8],
    /// `None` when `environmentSha256` of the item is null.
    pub environment: Option<&'a [u8]>,
}

impl std::fmt::Debug for Files<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Files")
            .field("compose_bytes", &self.compose.len())
            .field("environment_bytes", &self.environment.map(<[u8]>::len))
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pull {
    pub service: String,
    pub image: String,
    pub pull: Command,
}

/// One version whose files passed each rule; `{:?}` shows no byte or value of the files.
#[derive(PartialEq, Eq)]
pub struct Prepared {
    pub(crate) application: String,
    pub(crate) keep: bool,
    pub(crate) checked: Checked,
    pub(crate) compose: Vec<u8>,
    pub(crate) environment: Vec<u8>,
    /// The digests that the program has before a pull: those of the last apply when `pull`
    /// is false, those of the kept version for `application_back`.
    pub(crate) known: Vec<Pulled>,
    /// The pulls that the caller does before [`planned`]; none when `pull` is false.
    pub pulls: Vec<Pull>,
}

impl Drop for Prepared {
    fn drop(&mut self) {
        self.compose.zeroize();
        self.environment.zeroize();
    }
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Prepared")
            .field("application", &self.application)
            .field("keep", &self.keep)
            .field("compose_bytes", &self.compose.len())
            .field("environment_bytes", &self.environment.len())
            .field("known", &self.known)
            .field("pulls", &self.pulls)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub plan: Plan,
    /// The SHA-256 of the final form, for the log of the machine (Section 9).
    pub final_form_sha256: String,
    pub holds_volumes: bool,
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        // A write to a `String` does not fail.
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// The check of `application_apply` before the program has the two files.
///
/// # Errors
/// `applications_off`; `protected` when the list "Protected" has the project or one of its
/// containers; `name_taken` when a project of the name exists with a different folder
/// (SF-028); `busy`.
pub fn accepted(apply: &Apply, facts: &Facts<'_>) -> Result<(), Refusal> {
    project_checked(&apply.application, facts)
}

/// The project checks of an apply and a back, done before the files and again before the plan.
pub(crate) fn project_checked(application: &str, facts: &Facts<'_>) -> Result<(), Refusal> {
    managed(facts)?;
    let project = project_of(application);
    if project_is_protected(&project, facts.settings, facts.inventory) {
        return Err(Refusal::Protected);
    }
    let folder = facts.paths.application(application).files();
    let of_the_project = |container: &&crate::Container| container.is_of(&project);
    if facts
        .inventory
        .containers
        .iter()
        .filter(of_the_project)
        .any(|container| container.folder.as_deref() != Some(folder.as_str()))
    {
        return Err(Refusal::NameTaken);
    }
    free(facts)
}

fn images(checked: &Checked) -> Vec<(&str, &str)> {
    let services = checked.document.get("services").and_then(Node::entries);
    services
        .unwrap_or(&[])
        .iter()
        .filter_map(|(name, node)| Some((name.as_str(), node.get("image")?.text()?)))
        .collect()
}

fn pulls(checked: &Checked, facts: &Facts<'_>) -> Vec<Pull> {
    images(checked)
        .into_iter()
        .map(|(service, image)| Pull {
            service: service.to_owned(),
            image: image.to_owned(),
            pull: facts.paths.docker(&["image", "pull"], &[image]),
        })
        .collect()
}

pub(crate) fn final_form_of(
    prepared: &Prepared,
    digests: &[Pulled],
    facts: &Facts<'_>,
) -> Result<FinalForm, Refusal> {
    let project = project_of(&prepared.application);
    let binds = facts.paths.application(&prepared.application).binds();
    let own = facts.settings.limits.own(&facts.machine);
    let frame = Frame {
        project: &project,
        binds: &binds,
        own: &own,
        open_to_network: facts.settings.open_to_network,
        digests,
    };
    final_form::formed(&prepared.checked, &frame)
}

/// The check of the two files of `application_apply` (Sections 6.4.4 and 6.4.6).
///
/// # Errors
/// Each refusal of [`accepted`]; `program_mismatch` for a file whose SHA-256 is different
/// from the one of the item, and for an environment file that the item does not name;
/// `file_not_permitted` and `not_at_machine` for a file that breaks a rule of Section 6.4.4;
/// `limit_disk` when a pull is asked and the free disk is below the limit of the machine;
/// `no_digest` when `pull` is false and a service has no digest of the last apply.
pub fn prepared(apply: &Apply, files: &Files<'_>, facts: &Facts<'_>) -> Result<Prepared, Refusal> {
    accepted(apply, facts)?;
    let named = apply.environment_sha256.as_deref();
    let environment_sound = match (files.environment, named) {
        (None, None) => true,
        (Some(bytes), Some(hash)) => sha256(bytes) == hash,
        _ => false,
    };
    if sha256(files.compose) != apply.file_sha256 || !environment_sound {
        return Err(Refusal::ProgramMismatch);
    }
    let environment = files.environment.unwrap_or_default();
    let compose =
        std::str::from_utf8(files.compose).map_err(|_| Refusal::file(Rule::YamlNotPermitted))?;
    let variables =
        std::str::from_utf8(environment).map_err(|_| Refusal::file(Rule::VariableNotPermitted))?;
    let checked = compose_rules::check(compose, variables, &facts.settings.of_a_file())?;
    if apply.pull && facts.machine.free_disk_bytes < facts.settings.limits.free_disk_bytes {
        return Err(Refusal::LimitDisk);
    }
    let installed = facts.records.installed(&apply.application);
    let mut prepared = Prepared {
        application: apply.application.clone(),
        keep: apply.keep,
        pulls: Vec::new(),
        known: Vec::new(),
        compose: files.compose.to_vec(),
        environment: environment.to_vec(),
        checked,
    };
    if apply.pull {
        prepared.pulls = pulls(&prepared.checked, facts);
    } else {
        prepared.known = installed.map_or_default(|installed| installed.current.clone());
        final_form_of(&prepared, &prepared.known, facts)?;
    }
    Ok(prepared)
}

/// The plan of `application_apply`; `facts` are of the moment after the pulls, and the checks
/// of [`accepted`] run again with them.
///
/// # Errors
/// Each refusal of [`accepted`]; `no_digest` for a service that has no digest;
/// `file_not_permitted` for a value that the final form cannot hold.
pub fn planned(
    prepared: &Prepared,
    pulled: &[Pulled],
    facts: &Facts<'_>,
) -> Result<Planned, Refusal> {
    project_checked(&prepared.application, facts)?;
    let digests = if prepared.pulls.is_empty() {
        &prepared.known
    } else {
        pulled
    };
    plan_version(prepared, digests, facts, true)
}

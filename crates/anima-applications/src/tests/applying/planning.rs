use super::{COMPOSE, ENVIRONMENT, digests, plan, prepare};
use crate::{
    Refusal, Step,
    tests::{DIGEST, FILES, PROJECT, World, commands},
    work::apply::{Planned, planned, sha256},
};

#[test]
fn k10_6_4_4_a_service_with_no_digest_of_its_pull_is_never_started() {
    let world = World::new();
    let prepared = prepare(&world, COMPOSE).expect("the files pass");
    let facts = world.facts();
    assert_eq!(
        planned(&prepared, &digests()[..1], &facts).err(),
        Some(Refusal::NoDigest)
    );
    let mut wrong = digests();
    wrong[1].digest = "sha256:latest".to_owned();
    assert_eq!(
        planned(&prepared, &wrong, &facts).err(),
        Some(Refusal::NoDigest)
    );
    let other = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let by_digest = COMPOSE.replacen("postgres:17", &format!("postgres@{other}"), 1);
    let prepared = prepare(&world, &by_digest).expect("the files pass");
    let mut pulled = digests();
    pulled[1].image = format!("postgres@{other}");
    assert_eq!(
        planned(&prepared, &pulled, &facts).err(),
        Some(Refusal::NoDigest)
    );
    pulled[1].digest = other.to_owned();
    assert!(planned(&prepared, &pulled, &facts).is_ok());
}

#[test]
fn k10_6_4_5_the_plan_writes_the_files_below_files_and_makes_each_folder_of_binds() {
    let world = World::new();
    let planned = plan(&world, false);
    let application = "/var/lib/anima/applications/0192f6b0-7c1e-7a3b-9d2f-4e5a6b7c8d9e";
    let folders: Vec<&str> = planned
        .plan
        .steps
        .iter()
        .filter_map(|step| match step {
            Step::Folder { path } | Step::BindFolder { path } => Some(path.as_str()),
            _ => None,
        })
        .collect();
    let binds = format!("{application}/binds");
    assert_eq!(
        folders,
        [
            application,
            FILES,
            binds.as_str(),
            &format!("{binds}/config")
        ]
    );
    let written = |name: &str| {
        let path = format!("{FILES}/{name}");
        planned.plan.steps.iter().find_map(|step| match step {
            Step::Write { path: to, content } if *to == path => Some(content.clone()),
            _ => None,
        })
    };
    let version = version(&planned);
    assert_eq!(version.folders.files(), FILES);
    assert_eq!(version.compose, COMPOSE.as_bytes());
    assert_eq!(version.environment, ENVIRONMENT.as_bytes());
    assert_eq!(planned.final_form_sha256, sha256(&version.final_form));
    assert!(version.digests.starts_with(b"[{\"service\":\"app\""));
    let pulled =
        String::from_utf8(written("pulled.json").expect("the pulled images")).expect("text");
    assert_eq!(
        pulled,
        format!("[\"example/tracks@{DIGEST}\",\"postgres@{DIGEST}\"]\n")
    );
    for step in &planned.plan.steps {
        if let Step::Write { path, .. } | Step::Folder { path } = step {
            assert!(
                path.starts_with(application) && !path.ends_with("/.env"),
                "{path}"
            );
        }
    }
}

#[test]
fn k10_6_4_4_the_project_starts_from_the_final_form_only() {
    let world = World::new();
    let planned = plan(&world, false);
    let steps = &planned.plan.steps;
    let commands = commands(&planned.plan);
    assert_eq!(commands.len(), 1);
    let up = commands[0];
    assert_eq!(up.program, "/usr/libexec/docker/cli-plugins/docker-compose");
    let final_form = format!("{FILES}/final-form.json");
    assert_eq!(
        up.arguments,
        [
            "--project-name",
            PROJECT,
            "--project-directory",
            FILES,
            "--file",
            final_form.as_str(),
            "up",
            "-d",
            "--remove-orphans",
            "--pull",
            "never",
            "--no-build",
        ]
    );
    assert_eq!(up.folder.as_deref(), Some(FILES));
    assert!(
        !up.arguments
            .iter()
            .any(|argument| argument.contains("environment"))
    );
    let names: Vec<&str> = up
        .environment
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(names, ["DOCKER_HOST", "DOCKER_CONFIG"]);
    let last = steps
        .iter()
        .position(|step| matches!(step, Step::ApplyVersion { .. }))
        .expect("a start");
    let binds = FILES.replace("/files", "/binds");
    assert_eq!(
        steps[last - 1],
        Step::BindFolder {
            path: format!("{binds}/config")
        }
    );
    assert!(matches!(&steps[last - 2], Step::RealFolder { path, .. } if *path == binds));
    assert_eq!(
        steps[last - 3],
        Step::Absent {
            path: format!("{FILES}/.env")
        }
    );
}

fn version(planned: &Planned) -> &crate::plan::Version {
    planned
        .plan
        .steps
        .iter()
        .find_map(|step| match step {
            Step::ApplyVersion { version, .. } => Some(version.as_ref()),
            _ => None,
        })
        .expect("one application version")
}

#[test]
fn k10_6_4_6_keep_1_preserves_an_existing_version() {
    let mut world = World::new();
    assert!(version(&plan(&world, true)).keep_previous);
    assert!(!version(&plan(&world, false)).keep_previous);
    world.records.applications.clear();
    world.inventory.containers.remove(0);
    assert!(!version(&plan(&world, true)).keep_previous);
}

#[test]
fn k10_6_4_6_the_plan_checks_the_project_again_after_the_pulls() {
    let mut world = World::new();
    let prepared = prepare(&world, COMPOSE).expect("the files pass");
    world.settings.protected = vec![PROJECT.to_owned()];
    assert_eq!(
        planned(&prepared, &digests(), &world.facts()).err(),
        Some(Refusal::Protected)
    );
    world.settings.protected.clear();
    world.inventory.containers[0].folder = Some("/srv/elsewhere".to_owned());
    assert_eq!(
        planned(&prepared, &digests(), &world.facts()).err(),
        Some(Refusal::NameTaken)
    );
    world.inventory.containers[0].folder = Some(FILES.to_owned());
    world.busy = true;
    assert_eq!(
        planned(&prepared, &digests(), &world.facts()).err(),
        Some(Refusal::Busy)
    );
    world.busy = false;
    assert!(planned(&prepared, &digests(), &world.facts()).is_ok());
}

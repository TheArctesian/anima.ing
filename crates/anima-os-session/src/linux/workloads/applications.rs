//! Bind the observed Docker image to the applications owner's committed record.
use super::{Container, DOCKER, command, docker_args, valid_container};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

const FORMAT: &str = r#"{"id":{{json .Id}},"configuredImage":{{json .Config.Image}}}"#;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Image {
    id: String,
    configured_image: String,
}

pub(super) async fn mark(containers: &mut [Container]) {
    let records = records(containers).await;
    for container in containers
        .iter_mut()
        .filter(|container| container.application_candidate())
    {
        let record = records
            .as_ref()
            .and_then(|records| records.get(&container.id));
        container.application = record.cloned().flatten();
        container.application_record_unavailable = records.is_none() || record == Some(&None);
    }
}

async fn records(containers: &[Container]) -> Option<BTreeMap<String, Option<String>>> {
    let candidates: Vec<_> = containers
        .iter()
        .filter(|container| {
            container.application_candidate() && valid_container(&container.id).is_ok()
        })
        .collect();
    if candidates.is_empty() {
        return Some(BTreeMap::new());
    }
    let images = configured_images(&candidates).await;
    let observed: Vec<_> = candidates
        .into_iter()
        .map(|container| {
            json!({
                "id":container.id,"project":container.project,"service":container.service,
                "folder":container.folder,"finalForm":container.final_form,
                "configuredImage":images.get(&container.id),
            })
        })
        .collect();
    let input = serde_json::to_vec(&observed).ok()?;
    let response = command::run(
        "/run/current-system/sw/bin/anima-applications",
        &["console-applications", "/persist/anima-applications"],
        Some(&input),
        3,
    )
    .await
    .ok()?;
    serde_json::from_str(&response).ok()
}

async fn configured_images(candidates: &[&Container]) -> BTreeMap<String, String> {
    let mut args = vec!["container", "inspect", "--format", FORMAT];
    args.extend(candidates.iter().map(|container| container.id.as_str()));
    // If one container disappears, Docker can still return inspections of its siblings.
    let output = command::partial_stdout(DOCKER, &docker_args(&args), 5)
        .await
        .unwrap_or_default();
    output
        .lines()
        .filter_map(|line| serde_json::from_str::<Image>(line).ok())
        .filter(|image| !image.configured_image.is_empty())
        .map(|image| (image.id, image.configured_image))
        .collect()
}

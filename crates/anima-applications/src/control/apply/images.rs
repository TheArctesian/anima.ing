//! Resolve each successful Docker pull to the registry digest returned by that pull.
use std::time::Instant;

use crate::{
    Paths, Pulled, Refusal,
    execution::{Fault, check_socket, process},
    final_form::image_by_digest,
    forms::is_digest,
    work::apply::Pull,
};

use super::Failure;

const PULL_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

pub(super) fn pull(paths: &Paths, request: &Pull, deadline: Instant) -> Result<Pulled, Failure> {
    check_socket(paths)?;
    let output = process::capture_bounded(paths, &request.pull, deadline, PULL_OUTPUT_BYTES)?;
    let digest = pull_digest(&output).ok_or(Refusal::NoDigest)?;
    if request
        .image
        .split_once('@')
        .is_some_and(|(_, wanted)| wanted != digest)
    {
        return Err(Refusal::NoDigest.into());
    }

    let reference = image_by_digest(&request.image, digest);
    check_socket(paths)?;
    let command = paths.docker(&["image", "inspect", "--format", "{{.Id}}"], &[&reference]);
    let id = match process::capture(paths, &command, deadline) {
        Err(Fault::CommandFailed) => return Err(Refusal::NoDigest.into()),
        other => other?,
    };
    if !std::str::from_utf8(&id).is_ok_and(|text| is_digest(text.trim())) {
        return Err(Refusal::NoDigest.into());
    }
    Ok(Pulled {
        service: request.service.clone(),
        image: request.image.clone(),
        digest: digest.to_owned(),
    })
}

fn pull_digest(output: &[u8]) -> Option<&str> {
    let mut digests = std::str::from_utf8(output)
        .ok()?
        .lines()
        .filter_map(|line| line.strip_prefix("Digest: "));
    let digest = digests.next()?;
    (is_digest(digest) && digests.next().is_none()).then_some(digest)
}

//! Serialize the private digest records associated with a managed version.
use crate::{
    Pulled,
    final_form::image_by_digest,
    json::{self, Json},
};

use super::{Facts, Prepared};

pub(super) fn digests_written(digests: &[Pulled]) -> Vec<u8> {
    let one = |pulled: &Pulled| {
        Json::Record(vec![
            ("service".to_owned(), Json::text(&pulled.service)),
            ("image".to_owned(), Json::text(&pulled.image)),
            ("digest".to_owned(), Json::text(&pulled.digest)),
        ])
    };
    json::written(&Json::List(digests.iter().map(one).collect())).into_bytes()
}

pub(crate) const MAX_PULLED_BYTES: u64 = 1024 * 1024;

pub(super) fn pulled_written(
    prepared: &Prepared,
    digests: &[Pulled],
    facts: &Facts<'_>,
) -> Vec<u8> {
    let installed = facts.records.installed(&prepared.application);
    let mut pulled = installed.map_or_default(|installed| installed.pulled.clone());
    for one in digests {
        let image = image_by_digest(&one.image, &one.digest);
        if !pulled.contains(&image) {
            pulled.push(image);
        }
    }
    json::written(&Json::List(
        pulled.iter().map(|image| Json::text(image)).collect(),
    ))
    .into_bytes()
}

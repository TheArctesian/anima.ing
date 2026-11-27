//! Open the hub's local stores and publishing bucket at startup.
use std::sync::Arc;

use anima_hub::{
    config::Config,
    engine::{files, sealing_keys::SealingKeys},
    fleet::{builds, credentials::Vault},
};

use super::StartError;

pub(super) fn vault(config: &Config) -> Result<Vault, StartError> {
    Vault::from_key_file(&config.key_file).map_err(|source| StartError::Key {
        path: config.key_file.display().to_string(),
        source,
    })
}

/// A data key that is the credential key or another data key stops the start (VLT-003); without either folder an upload answers UNAVAILABLE.
pub(super) fn file_store(config: &Config, vault: &Vault) -> Result<files::Store, StartError> {
    let keys = SealingKeys::open(config.data_key_dir.as_deref(), vault.key_id().as_bytes())?;
    let store = files::Store::in_folder(config.files_dir.as_deref(), keys);
    tracing::info!(files = store.available(), "file store");
    Ok(store)
}

#[cfg(unix)]
pub(super) async fn artifact_store(
    config: Option<anima_hub::config::ArtifactStorage>,
    clock: Arc<dyn anima_hub::clock::Clock>,
) -> Result<Option<anima_hub::work::artifacts::Store>, StartError> {
    let Some(config) = config else {
        return Ok(None);
    };
    let store = tokio::task::spawn_blocking(move || {
        let store =
            anima_hub::work::artifacts::Store::open(&config.root, config.quota, config.reserve)?;
        match config.registry {
            Some(settings) => store.with_registry(&settings),
            None => Ok(store),
        }
    })
    .await
    .map_err(std::io::Error::other)??;
    tokio::spawn(reap_checkpoints(store.clone(), clock));
    Ok(Some(store))
}

#[cfg(unix)]
async fn reap_checkpoints(
    store: anima_hub::work::artifacts::Store,
    clock: Arc<dyn anima_hub::clock::Clock>,
) {
    let mut ticks = tokio::time::interval(std::time::Duration::from_secs(60));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticks.tick().await;
        match store.reap_checkpoints(clock.now()).await {
            Ok(()) | Err(anima_hub::work::artifacts::Error::Busy) => (),
            Err(error) => tracing::warn!(%error, "workspace checkpoint cleanup failed"),
        }
    }
}

/// An unusable updater key turns publishing off and the hub still starts (BLD-060, R-108, R-117).
/// The log names the check that failed, never a value (LOG-007).
pub(super) fn release_bucket(config: &Config) -> Result<Option<Arc<builds::Bucket>>, StartError> {
    let opened = config.releases.as_ref().map(builds::open_bucket);
    let bucket = opened.transpose()?.flatten();
    tracing::info!(publishing = bucket.is_some(), "release bucket");
    Ok(bucket.map(Arc::new))
}

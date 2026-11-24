//! The generator changes a recipe that exists only by a new digest or version (SF-244).

use serde_json::Value;

use crate::{
    Recipe,
    entry::{Entry, MachineTest, Reviewed},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Change {
    #[error("the recipe was written by a person")]
    Reviewed,
    #[error("the recipe cannot be read")]
    NotReadable,
    #[error("the change is more than a new digest or a new version, so a person reads it")]
    NotOnlyPin,
}

fn json(text: Option<&str>) -> Result<Value, Change> {
    text.and_then(|text| serde_json::from_str(text).ok())
        .ok_or(Change::NotReadable)
}

fn at<'a>(value: &'a mut Value, path: &[&str]) -> Option<&'a mut Value> {
    path.iter().try_fold(value, |node, key| node.get_mut(*key))
}

fn copy(into: &mut Value, from: &Value, path: &[&str]) -> Result<(), Change> {
    let new = path
        .iter()
        .try_fold(from, |node, key| node.get(*key))
        .ok_or(Change::NotOnlyPin)?
        .clone();
    *at(into, path).ok_or(Change::NotOnlyPin)? = new;
    Ok(())
}

fn images(entry: &Value) -> Vec<&str> {
    entry
        .pointer("/anima/images")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |images| {
            images
                .iter()
                .filter_map(|image| image.get("image").and_then(Value::as_str))
                .collect()
        })
}

fn entry(existing: &Recipe) -> Option<Entry> {
    (existing.entry.as_deref()).and_then(|text| serde_json::from_str::<Entry>(text).ok())
}

/// The review state, which the generator writes again unchanged (SF-255).
#[must_use]
pub fn reviewed(existing: &Recipe) -> Option<Reviewed> {
    entry(existing).and_then(|entry| entry.anima.reviewed)
}

#[must_use]
pub fn tested(existing: &Recipe) -> Option<MachineTest> {
    entry(existing).map(|entry| entry.anima.last_test)
}

/// Whether the generator may write `made` over `existing`.
///
/// # Errors
/// [`Change`] when a person wrote `existing`, an entry is not JSON, or more than the pin changed.
pub fn update(existing: &Recipe, made: &Recipe) -> Result<(), Change> {
    let mut old = json(existing.entry.as_deref())?;
    let new = json(made.entry.as_deref())?;
    if old.pointer("/anima/origin").is_none_or(Value::is_null) {
        return Err(Change::Reviewed);
    }
    let mut compose = existing.compose.clone().unwrap_or_default();
    let (old_images, new_images) = (images(&old), images(&new));
    if old_images.len() != new_images.len() {
        return Err(Change::NotOnlyPin);
    }
    for (before, after) in old_images.iter().zip(&new_images) {
        compose = compose.replace(before, after);
    }
    for path in [
        &["version"][..],
        &["anima", "pinned"],
        &["anima", "origin", "read"],
        &["anima", "images"],
    ] {
        copy(&mut old, &new, path)?;
    }
    if new.pointer("/anima/last_test") == Some(&serde_json::json!({ "result": "not-tested" })) {
        copy(&mut old, &new, &["anima", "last_test"])?;
    }
    if old == new && Some(compose) == made.compose && existing.environment == made.environment {
        Ok(())
    } else {
        Err(Change::NotOnlyPin)
    }
}

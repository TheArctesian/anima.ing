//! The members of an item (K10 Sections 4 and 5); an unknown member is refused, as it can narrow the item.

use serde_json::{Map, Value};

use super::permitted::{FORGES, is_name, is_owner};

type Object = Map<String, Value>;

const ITEM: [&str; 10] = [
    "id",
    "type",
    "kind",
    "attempt",
    "lease",
    "limits",
    "project",
    "repository",
    "secrets",
    "parameters",
];
const LIMITS: [&str; 5] = [
    "seconds",
    "idleSeconds",
    "memoryBytes",
    "diskBytes",
    "orphanSeconds",
];

pub(super) fn has_exactly(object: &Object, members: &[&str]) -> bool {
    object.len() == members.len() && members.iter().all(|member| object.contains_key(*member))
}

pub(super) fn object_of<'a>(
    parent: &'a Object,
    name: &str,
    members: &[&str],
) -> Option<&'a Object> {
    parent
        .get(name)?
        .as_object()
        .filter(|object| has_exactly(object, members))
}

pub(super) fn text<'a>(object: &'a Object, name: &str) -> Option<&'a str> {
    object.get(name)?.as_str()
}

fn limits_hold(item: &Object) -> bool {
    object_of(item, "limits", &LIMITS).is_some_and(|limits| {
        limits
            .values()
            .all(|limit| limit.as_u64().is_some_and(|count| count > 0))
    })
}

fn repository_holds(item: &Object) -> bool {
    object_of(item, "repository", &["forge", "owner", "name"]).is_some_and(|repository| {
        text(repository, "forge").is_some_and(|forge| FORGES.contains(&forge))
            && text(repository, "owner").is_some_and(is_owner)
            && text(repository, "name").is_some_and(is_name)
    })
}

fn project_holds(item: &Object) -> bool {
    match item.get("project") {
        Some(Value::Null) => true,
        Some(Value::String(id)) => uuid::Uuid::parse_str(id).is_ok(),
        _ => false,
    }
}

pub(super) fn purposes(item: &Object) -> Option<Vec<&str>> {
    item.get("secrets")?
        .as_array()?
        .iter()
        .map(Value::as_str)
        .collect()
}

/// The `parameters` object is checked by its work type.
pub(super) fn holds(item: &Object) -> bool {
    has_exactly(item, &ITEM)
        && object_of(item, "lease", &["until"]).is_some()
        && limits_hold(item)
        && repository_holds(item)
        && project_holds(item)
        && purposes(item).is_some()
        && item.get("parameters").is_some_and(Value::is_object)
}

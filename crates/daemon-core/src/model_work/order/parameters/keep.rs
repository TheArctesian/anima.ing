//! The members of K11 Section 11.1: `reserveBytes` and `keep` of a `model_load`, and the type `model_keep`.

use serde_json::Value;

use super::{Object, Parameters, exactly, settings_of, uuid};
use crate::model_work::cause::Cause;
use crate::model_work::memory::keep::{IDLE_SECONDS, Keep, MAX_RESERVE_BYTES};
use crate::model_work::order::ModelMachine;

/// `keep`: exactly `idleSeconds` (60 to 86,400) and `pinned`.
fn keep(value: &Value) -> Result<Keep, Cause> {
    let object = value.as_object().ok_or(Cause::MemberUnknown)?;
    exactly(object, &["idleSeconds", "pinned"])?;
    let idle_seconds = object
        .get("idleSeconds")
        .and_then(Value::as_u64)
        .and_then(|seconds| u32::try_from(seconds).ok())
        .filter(|seconds| IDLE_SECONDS.contains(seconds))
        .ok_or(Cause::MemberUnknown)?;
    let pinned = object
        .get("pinned")
        .and_then(Value::as_bool)
        .ok_or(Cause::MemberUnknown)?;
    Ok(Keep {
        idle_seconds,
        pinned,
    })
}

/// `model_load`: `server`, `copy` and `settings`, and the optional `reserveBytes` and `keep`.
pub(super) fn load(object: &Object, machine: &ModelMachine<'_>) -> Result<Parameters, Cause> {
    let optional = ["reserveBytes", "keep"];
    let required = ["server", "copy", "settings"];
    let holds = required.iter().all(|name| object.contains_key(*name))
        && object
            .keys()
            .all(|name| required.contains(&name.as_str()) || optional.contains(&name.as_str()));
    if !holds {
        return Err(Cause::MemberUnknown);
    }
    let reserve_bytes = object
        .get("reserveBytes")
        .map(|value| {
            value
                .as_u64()
                .filter(|bytes| (1..=MAX_RESERVE_BYTES).contains(bytes))
                .ok_or(Cause::MemberUnknown)
        })
        .transpose()?;
    let keep = object.get("keep").map(keep).transpose()?;
    let settings = settings_of(object, None, machine)?;
    Ok(Parameters::Load {
        server: uuid(object, "server")?,
        copy: uuid(object, "copy")?,
        settings,
        reserve_bytes,
        keep,
    })
}

/// `model_keep`: exactly `copy` and `keep`.
pub(super) fn model_keep(object: &Object) -> Result<Parameters, Cause> {
    exactly(object, &["copy", "keep"])?;
    Ok(Parameters::Keep {
        copy: uuid(object, "copy")?,
        keep: keep(object.get("keep").ok_or(Cause::MemberUnknown)?)?,
    })
}

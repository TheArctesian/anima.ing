//! Reclaim a released execution after its last executable grant expires.
use super::{Error, Phase, Result, store};
use chrono::Utc;
use daemon_core::source_jobs::Attempt;
use serde_json::Value;
use uuid::Uuid;

pub(super) fn forget((device, origin): (Uuid, &str), identity: Attempt) -> Result<Value> {
    if device.is_nil() || origin.is_empty() || identity.job.is_nil() || identity.attempt == 0 {
        return Err(Error::Refused("Choose a valid source attempt identity."));
    }
    let _lock = store::lock()?;
    if let Some(record) = store::read(identity.execution)?
        && (!record.coordinated
            || record.device != device
            || record.origin != origin
            || record.job.id != identity.job
            || record.job.attempt != identity.attempt
            || record.phase != Phase::Released
            || record.lease.grant.until > Utc::now())
    {
        return Err(Error::Refused("This source execution cannot be retired."));
    }
    if anima_os_maintenance::source_pipeline::active()?
        .is_some_and(|pipeline| pipeline.execution() == identity.execution)
    {
        return Err(Error::Refused(
            "The source admission still needs reconciliation.",
        ));
    }
    store::forget(identity.execution)?;
    Ok(serde_json::to_value(identity)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux::{
        builds::lease::{Grant, Lease},
        source_jobs::Record,
    };
    use chrono::TimeDelta;
    use serde_json::json;

    #[test]
    #[ignore = "requires a disposable root Linux container with empty /persist"]
    fn only_the_matching_released_attempt_can_be_forgotten_and_replayed() {
        std::fs::create_dir_all("/persist").unwrap();
        let device = Uuid::now_v7();
        let origin = "https://example.invalid";
        let identity = Attempt {
            execution: Uuid::now_v7(),
            job: Uuid::now_v7(),
            attempt: 1,
        };
        let mut record = released(device, origin, identity);
        let save = |record: &Record| {
            let _lock = store::lock().unwrap();
            store::save(record).unwrap();
        };
        save(&record);
        assert!(forget((device, origin), identity).is_err());

        record.lease.grant.until = Utc::now() - TimeDelta::seconds(1);
        record.phase = Phase::Ready;
        save(&record);
        assert!(forget((device, origin), identity).is_err());

        record.phase = Phase::Released;
        save(&record);
        assert!(forget((Uuid::now_v7(), origin), identity).is_err());
        assert!(
            forget(
                (device, origin),
                Attempt {
                    attempt: 2,
                    ..identity
                }
            )
            .is_err()
        );
        assert!(store::read(identity.execution).unwrap().is_some());

        let expected = serde_json::to_value(identity).unwrap();
        assert_eq!(forget((device, origin), identity).unwrap(), expected);
        assert!(store::read(identity.execution).unwrap().is_none());
        assert_eq!(forget((device, origin), identity).unwrap(), expected);
    }

    fn released(device: Uuid, origin: &str, identity: Attempt) -> Record {
        let until = Utc::now() + TimeDelta::seconds(60);
        let job = serde_json::from_value(json!({
            "id":identity.job,"type":"build","kind":"container","attempt":1,
            "lease":{"until":until.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)},
            "limits":{"seconds":300,"idleSeconds":60,"memoryBytes":536_870_912,
                "diskBytes":22_548_578_304_u64,"orphanSeconds":60},
            "project":null,"repository":{"forge":"github","owner":"example","name":"sample"},
            "secrets":[],"parameters":{"recipe":"source_image_v1","commit":"a".repeat(40),
                "target":"linux-aarch64","strategy":"auto","trigger":"operator","origin":"other"}
        }))
        .unwrap();
        Record {
            schema: 1,
            execution: identity.execution,
            device,
            origin: origin.into(),
            job,
            lease: Lease::new(Grant {
                job: identity.job,
                attempt: 1,
                until,
            })
            .unwrap(),
            registered_at: Utc::now(),
            coordinated: true,
            phase: Phase::Released,
            workspace: Uuid::now_v7(),
            context: Uuid::now_v7(),
            build: Uuid::now_v7(),
            result: None,
            failure: None,
            release_receipt: None,
        }
    }
}

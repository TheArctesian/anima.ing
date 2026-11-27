//! The cleanup intent precedes dispatch. No recovery signals a persisted numeric PID.

use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    config::{home::Home, private_file},
    remote_code::{Store, launch::Progress, wire::DispatchRequest},
};

const FILE: &str = "pending-remote-child.json";

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum Record {
    Current {
        request: DispatchRequest,
        boot: Option<Uuid>,
    },
    Legacy(Uuid),
}

impl Record {
    pub(super) fn new(request: DispatchRequest) -> Self {
        Self::Current {
            request,
            boot: boot_session(),
        }
    }

    pub(super) const fn runner(&self) -> Uuid {
        match self {
            Self::Current { request, .. } => request.runner,
            Self::Legacy(runner) => *runner,
        }
    }

    pub(super) fn ended(&self, dir: &Path) -> bool {
        let Self::Current { request, boot } = self else {
            return false;
        };
        if different_boot(*boot, boot_session()) {
            return true;
        }
        let Some(home) = dir.parent() else {
            return false;
        };
        // Dispatching is persisted before the OS spawn. Closed is reachable only from Reserved.
        Store::at(&Home::at(home.to_owned()))
            .reservation(request)
            .is_ok_and(|r| matches!(r.progress(), Progress::Reserved | Progress::Closed(_)))
    }
}

fn different_boot(previous: Option<Uuid>, current: Option<Uuid>) -> bool {
    previous
        .zip(current)
        .is_some_and(|(a, b)| !a.is_nil() && !b.is_nil() && a != b)
}

#[cfg(target_os = "macos")]
fn boot_session() -> Option<Uuid> {
    use std::{
        process::{Command, Stdio},
        sync::OnceLock,
    };
    static BOOT: OnceLock<Option<Uuid>> = OnceLock::new();
    *BOOT.get_or_init(|| {
        let output = Command::new("/usr/sbin/sysctl")
            .args(["-n", "kern.bootsessionuuid"])
            .env_clear()
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Uuid::parse_str(std::str::from_utf8(&output.stdout).ok()?.trim())
            .ok()
            .filter(|id| !id.is_nil())
    })
}

#[cfg(not(target_os = "macos"))]
const fn boot_session() -> Option<Uuid> {
    None
}

pub(super) fn save(dir: &Path, record: Option<&Record>) -> bool {
    let bytes = serde_json::to_vec(&record).expect("a pending request serializes");
    private_file::write(dir, FILE, &bytes)
        .inspect_err(|error| tracing::warn!(%error, "could not store pending remote cleanup"))
        .is_ok()
}

pub(super) fn read(dir: &Path) -> Result<Option<Record>, ()> {
    let Some(bytes) = private_file::read_within(&dir.join(FILE), 512).map_err(|_| ())? else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pre_dispatch_cleanup_intent_can_be_retired_but_a_created_child_stays_unresolved() {
        use crate::{
            config::origin::Origin,
            remote_code::{
                launch::{launch_fixture, started_fixture},
                wire::NotCreated,
            },
        };
        let (_tmp, home, claim, device) = launch_fixture();
        let dir = home.dir().join("threads");
        std::fs::create_dir(&dir).unwrap();
        let store = Store::at(&home);
        let original = store
            .reserve((device, &Origin::default_hub()), &claim)
            .unwrap();
        assert!(save(&dir, Some(&Record::new(*original.request()))));
        let pending = read(&dir).unwrap().unwrap();
        assert!(pending.ended(&dir));
        store
            .close(original.request(), NotCreated::Refused)
            .unwrap();
        assert!(pending.ended(&dir));
        let (_tmp, home, created, _) = started_fixture();
        let unresolved = Record::new(*created.request());
        assert!(!unresolved.ended(&home.dir().join("threads")));
    }

    #[test]
    fn recovery_requires_a_positive_boot_change_and_preserves_legacy_intent() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        assert!(different_boot(Some(a), Some(b)));
        assert!(!different_boot(Some(a), Some(a)));
        assert!(!different_boot(Some(a), None));
        assert!(!different_boot(Some(Uuid::nil()), Some(b)));
        let dir = tempfile::tempdir().unwrap();
        private_file::write(dir.path(), FILE, &serde_json::to_vec(&a).unwrap()).unwrap();
        let legacy = read(dir.path()).unwrap().unwrap();
        assert_eq!(legacy.runner(), a);
        assert!(!legacy.ended(dir.path()));
    }
}

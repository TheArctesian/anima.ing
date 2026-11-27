use chrono::{DateTime, Utc};
use daemon_core::wire::Lease;
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::super::{Error, Launch};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::fleet) struct ThreadWork {
    id: Uuid,
    #[serde(rename = "type")]
    work_type: &'static str,
    kind: &'static str,
    attempt: u32,
    lease: Lease,
    limits: Value,
    project: Uuid,
    parameters: Value,
}

impl ThreadWork {
    pub(super) fn for_launch(
        row: &Launch,
        account: Uuid,
        end: DateTime<Utc>,
    ) -> Result<Self, Error> {
        Ok(Self {
            id: row.work_id,
            work_type: "agent_thread",
            kind: "person",
            attempt: 1,
            lease: Lease { until: end },
            project: row.project.parse().map_err(|_| Error::malformed())?,
            limits: json!({ "seconds": 120, "idleSeconds": 0, "memoryBytes": 0, "diskBytes": 0, "orphanSeconds": 0 }),
            parameters: json!({
                "launch": row.id, "account": account, "program": row.program, "folder": row.folder, "access": row.access,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::{DateTime, Launch, Lease, ThreadWork, Utc, Uuid};

    #[test]
    fn claim_reply_uses_k8_lease_time() {
        #[derive(Deserialize)]
        struct ClaimLease {
            lease: Lease,
        }

        let until = "2026-11-25T17:23:45.123456Z"
            .parse::<DateTime<Utc>>()
            .unwrap();
        let row = Launch {
            id: Uuid::from_u128(1),
            work_id: Uuid::from_u128(2),
            device_id: Uuid::from_u128(3),
            first_command: Uuid::from_u128(4),
            fingerprint: vec![],
            key_version: 1,
            project: Uuid::from_u128(5).to_string(),
            folder: "/synthetic/project".into(),
            program: "codex".into(),
            access: "read_only".into(),
            deadline: until,
            claimed_at: None,
            lease_end: None,
            attempt_phase: None,
            runner_id: None,
            authorization: None,
            clock_generation: None,
            issued_tick: None,
            issued_at: None,
            valid_until: None,
            denial: None,
            evidence: None,
            admission_cause: None,
            publication: None,
            withdrawn: false,
            control_closed: false,
            device_removed_at: None,
            thread_id: None,
            retired: false,
        };
        let work = ThreadWork::for_launch(&row, Uuid::from_u128(6), until).unwrap();
        let body = serde_json::to_vec(&work).unwrap();
        let reply: ClaimLease = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            reply.lease.until,
            "2026-11-25T17:23:45Z".parse::<DateTime<Utc>>().unwrap()
        );
    }
}

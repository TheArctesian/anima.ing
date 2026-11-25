//! Authenticated daemon calls for K10 Section 6.5.

use daemon_core::wire::{DaemonToken, Reported, WorkState};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    Call, HubClient,
    answer::{Answer, Cause},
    read_json,
};
use crate::remote_code::{
    claim::Advertised,
    command::{Delivered, Receipt},
    wire::{Commands, ControlUpload, DispatchRequest, Evidence, Granted, LaunchReceipt},
};

const REPORT_LIMIT: usize = 4 * 1024;
const CONTROL_LIMIT: usize = 64 * 1024;
const POLL_LIMIT: usize = 256 * 1024;

enum Route {
    Dispatch,
    Report,
    Evidence,
    Poll,
    CommandResult,
    Control,
}

impl Route {
    const fn limits(self) -> (&'static str, usize, usize, &'static [u16]) {
        match self {
            Self::Dispatch => (
                "work/v1/thread-dispatch",
                REPORT_LIMIT,
                REPORT_LIMIT,
                &[200, 409],
            ),
            Self::Report => ("work/v1/report", REPORT_LIMIT, REPORT_LIMIT, &[200]),
            Self::Evidence => (
                "work/v1/thread-launch-evidence",
                REPORT_LIMIT,
                REPORT_LIMIT,
                &[200],
            ),
            Self::Poll => ("work/v1/commands", 8 * 1024, POLL_LIMIT, &[200, 204]),
            Self::CommandResult => ("work/v1/command-result", REPORT_LIMIT, REPORT_LIMIT, &[200]),
            Self::Control => (
                "work/v1/thread-control",
                CONTROL_LIMIT,
                CONTROL_LIMIT,
                &[204],
            ),
        }
    }

    fn call<'a>(self, token: &'a DaemonToken, message: &impl Serialize) -> Option<Call<'a>> {
        let (route, limit, response_limit, ok) = self.limits();
        let body = super::json(message);
        (body.len() <= limit).then_some(Call {
            route,
            bearer: Some(token),
            body,
            ok,
            response_limit,
        })
    }
}

/// A mixed K10 claim returns one classified item or no work.
pub enum Claimed {
    Empty,
    Work(Value),
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
pub enum DispatchDecision {
    Authorized { grant: Granted },
    Final { authorization: Option<Uuid> },
    Conflict,
}

impl HubClient {
    pub async fn claim_threads(
        &self,
        token: &DaemonToken,
        claim: &Advertised<'_>,
    ) -> Answer<Claimed> {
        let Some(body) = claim.encode() else {
            return Answer::Failed(Cause::Unreadable);
        };
        self.post(
            Call {
                route: "work/v1/claim",
                bearer: Some(token),
                body,
                ok: &[200, 204],
                response_limit: CONTROL_LIMIT,
            },
            |status, body| {
                if status == 204 {
                    Some(Claimed::Empty)
                } else {
                    read_json(body).map(Claimed::Work)
                }
            },
        )
        .await
    }

    /// Raw authenticated response. Only the holder's matching exchange can make this usable.
    pub async fn authorize_thread(
        &self,
        token: &DaemonToken,
        request: &DispatchRequest,
    ) -> Answer<DispatchDecision> {
        self.remote_post(Route::Dispatch.call(token, request), |status, body| {
            if status == 409 {
                return final_dispatch(body);
            }
            read_json::<Granted>(body).map(|grant| DispatchDecision::Authorized { grant })
        })
        .await
    }

    pub async fn prepare_thread(
        &self,
        token: &DaemonToken,
        request: &DispatchRequest,
    ) -> Answer<Reported> {
        let report = json!({ "id": request.work, "attempt": request.attempt,
            "state": WorkState::Preparing, "log": null, "result": null });
        self.remote_post(Route::Report.call(token, &report), |_, body| {
            read_json(body)
        })
        .await
    }

    pub async fn thread_launch_evidence(
        &self,
        token: &DaemonToken,
        evidence: &Evidence,
    ) -> Answer<LaunchReceipt> {
        self.remote_post(Route::Evidence.call(token, evidence), |_, body| {
            read_json(body)
        })
        .await
    }

    /// Use a separate client configured to allow the hub's 25-second hold.
    pub async fn poll_thread_commands(
        &self,
        token: &DaemonToken,
        device: Uuid,
    ) -> Answer<Vec<Delivered>> {
        self.remote_post(
            Route::Poll.call(token, &json!({ "device": device })),
            |status, body| {
                if status == 204 {
                    return Some(Vec::new());
                }
                let rows = read_json::<Commands>(body)?.commands;
                (rows.len() <= 32).then_some(rows)
            },
        )
        .await
    }

    pub async fn report_thread_command(
        &self,
        token: &DaemonToken,
        receipt: &Receipt,
    ) -> Answer<()> {
        self.remote_post(Route::CommandResult.call(token, receipt), |_, _| Some(()))
            .await
    }

    pub async fn upload_thread_control(
        &self,
        token: &DaemonToken,
        upload: &ControlUpload<'_>,
    ) -> Answer<()> {
        self.remote_post(Route::Control.call(token, upload), |_, _| Some(()))
            .await
    }

    async fn remote_post<T>(
        &self,
        call: Option<Call<'_>>,
        read: impl FnOnce(u16, &[u8]) -> Option<T>,
    ) -> Answer<T> {
        let Some(call) = call else {
            return Answer::Failed(Cause::Unreadable);
        };
        self.post(call, read).await
    }
}

fn final_dispatch(body: &[u8]) -> Option<DispatchDecision> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Final {
        error: String,
        #[serde(default, deserialize_with = "present_authorization")]
        authorization: Option<Uuid>,
    }
    let read: Final = read_json(body)?;
    match read.error.as_str() {
        "LAUNCH_CONFLICT" if read.authorization.is_none() => Some(DispatchDecision::Conflict),
        "NOT_HOLDER" if read.authorization.is_none_or(|id| !id.is_nil()) => {
            Some(DispatchDecision::Final {
                authorization: read.authorization,
            })
        }
        _ => None,
    }
}

fn present_authorization<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<Uuid>, D::Error> {
    Uuid::deserialize(decoder).map(Some)
}

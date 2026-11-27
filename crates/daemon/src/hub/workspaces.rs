//! Workspace lifecycle uses the enrolled node credential over the existing hub client.
use super::{
    Call, HubClient,
    answer::{Answer, Cause},
    json, read_json,
};
use agent_threads::managed::workspace::{Capacity, CapacityPoll, Delivery, Poll};
use daemon_core::wire::DaemonToken;

impl HubClient {
    pub async fn poll_workspaces(
        &self,
        token: &DaemonToken,
        poll: &Poll,
        (capacity, private_checkout): (Option<Capacity>, bool),
    ) -> Answer<Delivery> {
        if let Some(capacity) = capacity {
            let routes = [
                (private_checkout && self.local.is_none()).then_some("daemon/v3/workspaces/poll"),
                Some("daemon/v2/workspaces/poll"),
            ];
            for route in routes.into_iter().flatten() {
                let answer = self
                    .post(
                        Call {
                            route,
                            bearer: Some(token),
                            body: json(&CapacityPoll {
                                device: poll.device,
                                ready: poll.ready,
                                report: poll.report.clone(),
                                capacity,
                            }),
                            ok: &[200],
                        },
                        |_, body| read_json(body),
                    )
                    .await;
                if !matches!(answer, Answer::Failed(Cause::Status(404 | 405))) {
                    return answer;
                }
            }
        }
        self.post(
            Call {
                route: "daemon/v1/workspaces/poll",
                bearer: Some(token),
                body: json(poll),
                ok: &[200],
            },
            |_, body| read_json(body),
        )
        .await
    }
}

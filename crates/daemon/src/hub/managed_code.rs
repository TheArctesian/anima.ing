//! Managed Code uses the enrolled node credential over the existing hub client.
use super::{
    Call, HubClient,
    answer::{Answer, Cause},
    json, read_json,
};
use agent_threads::managed::{Delivery, MAX_POLL_BYTES, servers::ServerPoll};
use daemon_core::wire::DaemonToken;

impl HubClient {
    pub async fn poll_managed_code(
        &self,
        token: &DaemonToken,
        request: &ServerPoll,
    ) -> Answer<Delivery> {
        let with_servers = (!request.servers.is_empty())
            .then(|| json(request))
            .filter(|body| body.len() <= MAX_POLL_BYTES);
        if let Some(body) = with_servers {
            let answer = self
                .post(
                    Call {
                        route: "daemon/v2/code/poll",
                        bearer: Some(token),
                        body,
                        response_limit: crate::hub::body::LIMIT,
                        ok: &[200],
                    },
                    |_, body| read_json(body),
                )
                .await;
            if !matches!(answer, Answer::Failed(Cause::Status(404 | 405))) {
                return answer;
            }
        }
        self.post(
            Call {
                route: "daemon/v1/code/poll",
                bearer: Some(token),
                body: json(&request.poll),
                response_limit: crate::hub::body::LIMIT,
                ok: &[200],
            },
            |_, body| read_json(body),
        )
        .await
    }
}

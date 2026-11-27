//! Workspace checkpoint control and streaming from the enrolled hub.
mod exchange;

use super::{Call, HubClient, answer::Answer, json, read_json};
use agent_threads::managed::workspace::handoff::{Import, Offer, Receipt, Source, Withdrawal};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use daemon_core::wire::DaemonToken;
use exchange::{Response, request};
use hyper::http::{HeaderValue, Method, StatusCode};

pub(crate) use exchange::Stream;

const POLL: &str = "daemon/v1/workspace-handoffs/poll";
const HEADER: &str = "anima-workspace-checkpoint";

impl HubClient {
    pub(crate) async fn poll_handoffs(&self, token: &DaemonToken) -> Option<Offer> {
        match self
            .post(
                Call {
                    route: POLL,
                    bearer: Some(token),
                    body: json(&()),
                    ok: &[200, 204],
                },
                |status, bytes| {
                    if status == 204 {
                        Some(None)
                    } else {
                        read_json(bytes).map(Some)
                    }
                },
            )
            .await
        {
            Answer::Accepted(offer) => offer,
            _ => None,
        }
    }

    pub(crate) async fn upload_checkpoint(
        &self,
        token: &DaemonToken,
        receipt: &Receipt,
        stream: Stream,
    ) -> bool {
        let Some(header) = receipt_header(receipt) else {
            return false;
        };
        let mut request = request(Method::POST, receipt.source.binding.transfer, token);
        request.headers_mut().insert(HEADER, header);
        let Some(response) = self.checkpoint(request, Some(stream)).await else {
            return false;
        };
        response.status == StatusCode::ACCEPTED
            && response.receipt().await.as_ref() == Some(receipt)
    }

    pub(crate) async fn download_checkpoint(
        &self,
        token: &DaemonToken,
        receipt: &Receipt,
    ) -> Option<Response> {
        let request = request(Method::GET, receipt.source.binding.transfer, token);
        let response = self.checkpoint(request, None).await?;
        (response.status == StatusCode::OK && response.length == Some(receipt.bytes))
            .then_some(response)
    }

    pub(crate) async fn confirm_checkpoint(&self, token: &DaemonToken, receipt: &Receipt) -> bool {
        self.checkpoint_report(token, (receipt.source.binding.transfer, "confirm"), receipt)
            .await
    }

    pub(crate) async fn report_checkpoint_restored(
        &self,
        token: &DaemonToken,
        import: &Import,
    ) -> bool {
        self.checkpoint_report(
            token,
            (import.receipt.source.binding.transfer, "restored"),
            import,
        )
        .await
    }

    pub(crate) async fn report_checkpoint_activated(
        &self,
        token: &DaemonToken,
        import: &Import,
    ) -> bool {
        self.checkpoint_report(
            token,
            (import.receipt.source.binding.transfer, "activated"),
            import,
        )
        .await
    }

    pub(crate) async fn report_withdrawal(&self, token: &DaemonToken, result: &Withdrawal) -> bool {
        self.checkpoint_report(token, (result.transfer(), "withdrawal"), result)
            .await
    }

    pub(crate) async fn report_retirement(&self, token: &DaemonToken, source: Source) -> bool {
        self.checkpoint_report(token, (source.binding.transfer, "retired"), &source)
            .await
    }

    async fn checkpoint_report(
        &self,
        token: &DaemonToken,
        (transfer, operation): (uuid::Uuid, &str),
        report: &impl serde::Serialize,
    ) -> bool {
        let request = exchange::control(operation, transfer, token);
        let Ok(body) = serde_json::to_vec(report) else {
            return false;
        };
        self.checkpoint(request, Some(exchange::single(body)))
            .await
            .is_some_and(|response| response.status == StatusCode::NO_CONTENT)
    }
}

fn receipt_header(receipt: &Receipt) -> Option<HeaderValue> {
    let encoded = URL_SAFE_NO_PAD.encode(serde_json::to_vec(receipt).ok()?);
    (encoded.len() <= 4096)
        .then(|| encoded.parse().ok())
        .flatten()
}

use axum::http::{HeaderMap, header::AUTHORIZATION};
use chrono::{TimeDelta, Utc};
use daemon_core::wire::{EnrolRequest, Enrolled, Renewed};
use tokio::sync::Notify;

use super::{DEVICE, DaemonToken, Hub, Json, Router, State, post, protected};

#[derive(Default)]
pub(super) struct Renewal {
    allow: Notify,
    observed: Notify,
}

impl Renewal {
    pub(super) async fn allow_and_observe(&self) {
        self.allow.notify_one();
        tokio::time::timeout(std::time::Duration::from_secs(40), self.observed.notified())
            .await
            .expect("the rotated credential should reach the protected K8 publisher");
    }
}

pub(super) fn routes(router: Router<Hub>) -> Router<Hub> {
    router
        .route("/daemon/v1/enrol", post(enrol))
        .route("/daemon/v1/renew", post(renew))
}

async fn enrol(State(hub): State<Hub>, Json(request): Json<EnrolRequest>) -> Json<Enrolled> {
    assert!(
        request.code.expose() == protected::CODE,
        "unexpected fixture enrollment code"
    );
    Json(Enrolled {
        device: DEVICE,
        device_name: "fixture".to_owned(),
        token: DaemonToken::from_random([3; 32]),
        expires_at: Utc::now()
            + if hub.3.is_some() {
                TimeDelta::days(30) + TimeDelta::seconds(15)
            } else {
                TimeDelta::days(180)
            },
    })
}

async fn renew(State(hub): State<Hub>, headers: HeaderMap) -> Json<Renewed> {
    assert!(
        uses(&headers, 3),
        "incorrect credential for fixture renewal"
    );
    hub.3
        .as_ref()
        .expect("this fixture does not renew")
        .allow
        .notified()
        .await;
    Json(Renewed {
        token: DaemonToken::from_random([4; 32]),
        expires_at: Utc::now() + TimeDelta::days(180),
    })
}

pub(super) fn observe_renewed(renewal: Option<&Renewal>, headers: &HeaderMap) {
    if let Some(renewal) = renewal
        && uses(headers, 4)
    {
        renewal.observed.notify_one();
    }
}

fn uses(headers: &HeaderMap, marker: u8) -> bool {
    let token = DaemonToken::from_random([marker; 32]);
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        == Some(format!("Bearer {}", token.expose()).as_str())
}

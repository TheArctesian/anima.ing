use daemon_core::wire::RefusalCode;

use crate::{
    config::home::Home,
    hub::answer::Answer,
    remote_code::{Store, delivery::Delivery, launch::PublicationProgress, wire::DispatchRequest},
};

use super::super::Runner;

impl Runner {
    pub(super) fn remote_publish(&mut self, request: DispatchRequest) {
        let home = self.remote_home();
        let Some(row) = self
            .threads
            .get(&request.runner)
            .filter(|thread| {
                thread.remote_ready()
                    || !crate::runtime::thread_sync::remote_publishable(&home, &thread.row)
            })
            .map(|thread| thread.row.clone())
        else {
            return;
        };
        let now = tokio::time::Instant::now();
        if self
            .remote
            .publications
            .get(&request.runner)
            .is_some_and(|at| *at > now)
        {
            return;
        }
        self.remote
            .publications
            .insert(request.runner, now + std::time::Duration::from_secs(10));
        tokio::spawn(async move {
            if let Some(delivery) = super::polling::enrolled(home.clone()).await {
                publish(&home, &delivery, request, row).await;
            }
        });
    }
}

async fn publish(
    home: &Home,
    delivery: &Delivery,
    request: DispatchRequest,
    row: agent_threads::control::ThreadRow,
) {
    let store = Store::at(home);
    let allowed = || crate::runtime::thread_sync::remote_publishable(home, &row);
    if allowed()
        && delivery.publication(home, &request) == Some(PublicationProgress::Accepted)
        && delivery.remains_enrolled(home).await
        && allowed()
    {
        let reported = delivery.report_started(home, &request, allowed).await;
        let settled = match reported {
            Answer::Accepted(_) => true,
            Answer::Refused(RefusalCode::NotHolder) if allowed() => {
                matches!(delivery.evidence(&request).await, Answer::Accepted(_))
            }
            _ => false,
        };
        if settled {
            let _ = store.publication_reported(&request, delivery.credential_stamp());
        }
    }
    if !allowed() && store.withdraw(&request).is_ok() {
        let _ = delivery.evidence(&request).await;
    }
}

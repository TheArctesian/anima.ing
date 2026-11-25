use crate::{
    config::home::Home,
    remote_code::{Store, delivery::Delivery, wire::DispatchRequest},
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
    let allowed = || crate::runtime::thread_sync::remote_publishable(home, &row);
    if allowed() && delivery.publish_summary(home, &row, &request).await && allowed() {
        let _ = delivery.report_started(&request).await;
    }
    if !allowed() && Store::at(home).withdraw(&request).is_err() {
        return;
    }
    let _ = delivery.evidence(&request).await;
}

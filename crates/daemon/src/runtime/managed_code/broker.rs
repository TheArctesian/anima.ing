use agent_threads::managed::{BROKER_SOCKET, LocalRequest, servers::Inventory};
use serde_json::Value;

pub(in crate::runtime) async fn call(request: &LocalRequest) -> Option<Value> {
    super::super::owner_socket::call(BROKER_SOCKET, request, (65536, 1_100_000)).await
}

pub(super) async fn inventory() -> Option<Inventory> {
    if let Some(inventory) = call(&LocalRequest::OffersWithServers)
        .await
        .and_then(|value| serde_json::from_value(value).ok())
    {
        return Some(inventory);
    }
    let legacy = call(&LocalRequest::Offers).await?;
    Some(Inventory {
        workspaces: serde_json::from_value(legacy).ok()?,
        servers: Vec::new(),
    })
}

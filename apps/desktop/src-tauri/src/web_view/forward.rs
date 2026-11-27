//! Main-view-owned IPv4 loopback listeners for private workspace previews.
mod transport;
use super::{WebUrl, address::require_preview_url, commands::caller};
use agent_threads::managed::preview::FORWARD_PATH;
use std::{
    collections::HashMap,
    io::ErrorKind,
    net::Ipv4Addr,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};
use tauri::{Emitter as _, EventTarget, Manager, Webview};
use tokio::{net::TcpListener, sync::oneshot};
use uuid::Uuid;
use zeroize::Zeroizing;

type Slots = HashMap<Uuid, Option<oneshot::Sender<()>>>;

#[derive(Clone, Default)]
pub struct Forwards(Arc<Mutex<Slots>>);

struct Open {
    owner: Forwards,
    id: Uuid,
}

impl Drop for Open {
    fn drop(&mut self) {
        self.owner.slots().remove(&self.id);
    }
}

impl Forwards {
    fn slots(&self) -> MutexGuard<'_, Slots> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn reserve(&self, id: Uuid) -> Result<(Open, oneshot::Receiver<()>), &'static str> {
        let mut slots = self.slots();
        if slots.len() >= 8 || slots.contains_key(&id) {
            return Err("forward_limit");
        }
        let (cancel, requested) = oneshot::channel();
        slots.insert(id, Some(cancel));
        Ok((
            Open {
                owner: self.clone(),
                id,
            },
            requested,
        ))
    }

    fn stop(&self, id: Uuid) {
        if let Some(cancel) = self.slots().get_mut(&id).and_then(Option::take) {
            let _ = cancel.send(());
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Opening {
    id: Uuid,
    endpoint: Uuid,
    action: String,
    token: String,
    local_port: u16,
}

#[tauri::command]
pub async fn workspace_forward_start(
    webview: Webview,
    opening: Opening,
) -> Result<String, &'static str> {
    let Opening {
        id,
        endpoint,
        action,
        token,
        local_port,
    } = opening;
    caller(webview.label()).map_err(|_| "caller")?;
    let token = Zeroizing::new(token);
    if id.is_nil() || !transport::valid_secret(&token) || local_port == 0 {
        return Err("invalid_forward");
    }
    let action = WebUrl::parse(&action).map_err(|_| "invalid_forward")?;
    require_preview_url(action.as_url(), endpoint, "/.anima/handoff")
        .map_err(|_| "invalid_forward")?;
    let mut url = action.as_url().clone();
    url.set_path(FORWARD_PATH);
    let (slot, mut stopping) = webview.state::<Forwards>().reserve(id)?;
    let listener = bind_loopback(local_port).await?;
    let connection = tokio::select! {
        _ = &mut stopping => return Err("forward_cancelled"),
        result = transport::Connection::redeem(url, &token) => result?,
    };
    tauri::async_runtime::spawn(async move {
        let _slot = slot;
        tokio::select! {
            _ = stopping => (),
            () = connection.serve(listener) => (),
        }
        let _ = webview.emit_to(
            EventTarget::webview(super::event::MAIN),
            "workspace-forward-ended",
            id,
        );
    });
    Ok(format!("http://127.0.0.1:{local_port}"))
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri extracts the webview by value"
)]
pub fn workspace_forward_stop(webview: Webview, id: Uuid) -> Result<(), &'static str> {
    caller(webview.label()).map_err(|_| "caller")?;
    webview.state::<Forwards>().stop(id);
    Ok(())
}

async fn bind_loopback(port: u16) -> Result<TcpListener, &'static str> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|error| {
            if error.kind() == ErrorKind::AddrInUse {
                "port_in_use"
            } else {
                "port_unavailable"
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_forward_binds_only_loopback_and_never_reuses_an_occupied_port() {
        let listener = bind_loopback(0).await.unwrap();
        let address = listener.local_addr().unwrap();
        assert_eq!(address.ip(), Ipv4Addr::LOCALHOST);
        assert_eq!(
            bind_loopback(address.port()).await.unwrap_err(),
            "port_in_use"
        );
    }
}
